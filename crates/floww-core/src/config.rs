//! Two config layers.
//!
//! - `brain/_floww/team.toml`: rules the whole team shares (via the brain branch).
//! - `.floww/workspace.toml`: this machine only (database, port, AI auth, models).
//!
//! The effective config is the team config with local overrides applied.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const TEAM_CONFIG_PATH: &str = ".floww/brain/_floww/team.toml";
pub const WORKSPACE_CONFIG_PATH: &str = ".floww/workspace.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Odoo,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitFormat {
    /// `[FIX] module: summary`
    Odoo,
    /// `fix(module): summary`
    Conventional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StepSettings {
    pub skills: Vec<String>,
    /// Time limit for one AI session in this step.
    pub max_minutes: u32,
    /// Context pack budget in tokens for this step.
    pub context_tokens: u32,
}

impl StepSettings {
    fn with(skills: &[&str], context_tokens: u32) -> Self {
        Self {
            skills: skills.iter().map(|s| s.to_string()).collect(),
            context_tokens,
            ..Self::default()
        }
    }
}

impl Default for StepSettings {
    fn default() -> Self {
        Self {
            skills: Vec::new(),
            max_minutes: 30,
            context_tokens: 6_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Steps {
    pub prd: StepSettings,
    pub develop: StepSettings,
    pub test: StepSettings,
    pub commit: StepSettings,
    pub brain: StepSettings,
    pub full_suite: bool,
}

impl Default for Steps {
    fn default() -> Self {
        Self {
            prd: StepSettings::with(&["grill-me"], 6_000),
            // One session builds the whole PRD, so it gets more time.
            develop: StepSettings {
                max_minutes: 60,
                ..StepSettings::with(&["tdd", "sdd"], 10_000)
            },
            test: StepSettings::with(&[], 6_000),
            commit: StepSettings::with(&[], 3_000),
            brain: StepSettings::with(&[], 3_000),
            full_suite: false,
        }
    }
}

/// Shared by the team through the brain branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TeamConfig {
    pub preset: Preset,
    /// Language the AI writes PRDs and brain notes in (`id`, `en`).
    pub language: String,
    pub base_branch: String,
    pub feature_branch: String,
    pub commit_format: CommitFormat,
    pub steps: Steps,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            preset: Preset::Odoo,
            language: "id".into(),
            base_branch: "preprod".into(),
            feature_branch: crate::branch::DEFAULT_TEMPLATE.into(),
            commit_format: CommitFormat::Odoo,
            steps: Steps::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum AiAuth {
    /// Use the login already stored by Claude Code.
    Subscription,
    /// Route Claude Code through a 9router instance.
    Router {
        url: String,
        /// Keyring reference, e.g. `keyring:floww-9router`. Never the raw token.
        token_ref: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Models {
    pub default: Option<String>,
    pub prd: Option<String>,
    pub develop: Option<String>,
    pub brain: Option<String>,
    pub commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub name: String,
    /// Prefix for PRD ids, e.g. `SP` gives `SP-012`.
    pub initials: String,
    pub db: String,
    pub http_port: u16,
    pub launch_config: Option<String>,
    pub github_token_ref: Option<String>,
    /// `None` means: use the app-wide auth chosen on first run.
    pub ai: Option<AiAuth>,
    pub models: Models,
    /// Local overrides of team settings. Only set keys win.
    pub overrides: TeamOverrides,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            initials: String::new(),
            db: String::new(),
            http_port: 8069,
            launch_config: None,
            github_token_ref: None,
            ai: None,
            models: Models::default(),
            overrides: TeamOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TeamOverrides {
    pub base_branch: Option<String>,
    pub language: Option<String>,
    pub full_suite: Option<bool>,
}

/// App-wide settings, stored in `~/.config/floww/app.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    /// `None` until the first-run screen is finished.
    pub ai: Option<AiAuth>,
    pub default_model: Option<String>,
    pub workspaces: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub id: String,
    pub name: String,
    pub path: std::path::PathBuf,
}

/// What the app actually uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectiveConfig {
    pub team: TeamConfig,
    pub workspace: WorkspaceConfig,
}

impl EffectiveConfig {
    pub fn merge(mut team: TeamConfig, workspace: WorkspaceConfig) -> Self {
        let o = &workspace.overrides;
        if let Some(v) = &o.base_branch {
            team.base_branch = v.clone();
        }
        if let Some(v) = &o.language {
            team.language = v.clone();
        }
        if let Some(v) = o.full_suite {
            team.steps.full_suite = v;
        }
        Self { team, workspace }
    }

    /// Model for a step: step override, else the workspace default.
    pub fn model_for(&self, step: ModelStep) -> Option<&str> {
        let m = &self.workspace.models;
        let specific = match step {
            ModelStep::Prd => &m.prd,
            ModelStep::Develop => &m.develop,
            ModelStep::Brain => &m.brain,
            ModelStep::Commit => &m.commit,
        };
        specific.as_deref().or(m.default.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelStep {
    Prd,
    Develop,
    Brain,
    Commit,
}

/// Read a TOML file, or return the default when it does not exist yet.
pub fn read_toml_or_default<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| Error::Toml {
            path: path.to_path_buf(),
            message: e.message().to_string(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(source) => Err(Error::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn write_toml<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let text = toml::to_string_pretty(value).map_err(|e| Error::Invalid {
        field: "config",
        message: e.to_string(),
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, text).map_err(|source| Error::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Load the effective config for a repository folder.
pub fn load(repo: &Path) -> Result<EffectiveConfig> {
    let team: TeamConfig = read_toml_or_default(&repo.join(TEAM_CONFIG_PATH))?;
    let workspace: WorkspaceConfig = read_toml_or_default(&repo.join(WORKSPACE_CONFIG_PATH))?;
    Ok(EffectiveConfig::merge(team, workspace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_give_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.team.base_branch, "preprod");
        assert_eq!(cfg.team.steps.develop.skills, vec!["tdd", "sdd"]);
        assert_eq!(cfg.workspace.ai, None);
    }

    #[test]
    fn local_overrides_win_over_team() {
        let team = TeamConfig::default();
        let workspace = WorkspaceConfig {
            overrides: TeamOverrides {
                base_branch: Some("staging".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let cfg = EffectiveConfig::merge(team, workspace);
        assert_eq!(cfg.team.base_branch, "staging");
        assert_eq!(cfg.team.language, "id");
    }

    #[test]
    fn model_falls_back_to_default() {
        let workspace = WorkspaceConfig {
            models: Models {
                default: Some("claude-sonnet-5-5".into()),
                develop: Some("claude-opus-5-5".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let cfg = EffectiveConfig::merge(TeamConfig::default(), workspace);
        assert_eq!(cfg.model_for(ModelStep::Develop), Some("claude-opus-5-5"));
        assert_eq!(cfg.model_for(ModelStep::Commit), Some("claude-sonnet-5-5"));
    }

    #[test]
    fn round_trips_through_files() {
        let dir = tempfile::tempdir().unwrap();
        let ws = WorkspaceConfig {
            name: "acme-odoo".into(),
            initials: "SP".into(),
            db: "acme_dev".into(),
            ai: Some(AiAuth::Router {
                url: "http://localhost:20128".into(),
                token_ref: "keyring:floww-9router".into(),
            }),
            ..Default::default()
        };
        write_toml(&dir.path().join(WORKSPACE_CONFIG_PATH), &ws).unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.workspace, ws);
    }

    #[test]
    fn bad_toml_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(WORKSPACE_CONFIG_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "name = ").unwrap();
        let err = load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("workspace.toml"), "{err}");
    }
}
