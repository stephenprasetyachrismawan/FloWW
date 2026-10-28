//! Workspace setup: look at the repo and the machine, fill in what can be
//! known, and list what is unclear so the person can choose.

use std::path::Path;

use floww_core::odoo::{self, OdooLaunch};
use serde::Serialize;

use crate::{db, git, odoo_facts, proc};

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub key: String,
    pub question: String,
    pub options: Vec<ChoiceOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChoiceOption {
    pub value: String,
    pub why: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    pub repo: String,
    pub name: String,
    pub is_git: bool,
    pub preset: String,
    pub module_count: usize,
    pub launches: Vec<OdooLaunch>,
    pub databases: Vec<String>,
    pub branches: Vec<String>,
    pub default_branch: Option<String>,
    pub has_preprod: bool,
    pub has_remote: bool,
    pub initials: String,
    pub claude_installed: bool,
    pub graphify_installed: bool,
    pub checks: Vec<Check>,
    /// Things the person should decide. Each has a recommended option.
    pub choices: Vec<Choice>,
    /// Suggested values for the review screen.
    pub suggested: Suggested,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Suggested {
    pub base_branch: String,
    pub db: String,
    pub launch_config: Option<String>,
    pub http_port: u16,
}

pub async fn detect(repo: &Path, used_ports: &[u16]) -> Detection {
    let name = repo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "workspace".into());
    let is_git = git::is_repo(repo).await;
    let modules = odoo_facts::module_dirs(repo);
    let launch_path = repo.join(".vscode").join("launch.json");
    let launches = if launch_path.exists() {
        odoo::read_launch_configs(&launch_path).unwrap_or_default()
    } else {
        Vec::new()
    };
    let databases = db::list_databases().await.unwrap_or_default();
    let branches = if is_git {
        git::branches(repo).await
    } else {
        Vec::new()
    };
    let has_remote = is_git && git::has_remote(repo).await;
    let has_preprod = is_git
        && (git::local_branch_exists(repo, "preprod").await
            || git::remote_branch_exists(repo, "preprod").await);
    let default_branch = if is_git {
        git::default_branch(repo).await
    } else {
        None
    };
    let user = if is_git {
        git::user_name(repo).await
    } else {
        None
    };
    let initials = user
        .as_deref()
        .map(floww_core::prd_id::initials_from_name)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "ME".into());
    let claude_installed = proc::which(&crate::runner::claude_program()).is_some()
        || Path::new(&crate::runner::claude_program()).is_file();
    let graphify_installed = proc::which("graphify").is_some();

    let mut checks = vec![
        Check {
            label: "Git repository".into(),
            ok: is_git,
            detail: if is_git {
                "Found".into()
            } else {
                "This folder is not a git repository".into()
            },
        },
        Check {
            label: "Odoo modules".into(),
            ok: !modules.is_empty(),
            detail: format!("{} folders with __manifest__.py", modules.len()),
        },
        Check {
            label: "launch.json".into(),
            ok: !launches.is_empty(),
            detail: if launches.is_empty() {
                "No odoo-bin configuration in .vscode/launch.json".into()
            } else {
                format!("{} odoo-bin configurations", launches.len())
            },
        },
        Check {
            label: "Postgres".into(),
            ok: !databases.is_empty(),
            detail: if databases.is_empty() {
                "Could not list databases with psql".into()
            } else {
                format!("{} databases", databases.len())
            },
        },
        Check {
            label: "Claude Code".into(),
            ok: claude_installed,
            detail: if claude_installed {
                "Installed".into()
            } else {
                "Install Claude Code and sign in".into()
            },
        },
        Check {
            label: "graphify".into(),
            ok: graphify_installed,
            detail: if graphify_installed {
                "Installed".into()
            } else {
                "Optional. FloWW's own Odoo graph is used".into()
            },
        },
    ];
    checks.push(Check {
        label: "Branch preprod".into(),
        ok: has_preprod,
        detail: if has_preprod {
            "Found".into()
        } else {
            "Will be created from the default branch".into()
        },
    });

    let mut choices = Vec::new();
    if launches.len() > 1 {
        choices.push(Choice {
            key: "launch_config".into(),
            question: format!(
                "I found {} Odoo launch configurations. Which one is for this project?",
                launches.len()
            ),
            options: launches
                .iter()
                .enumerate()
                .map(|(i, l)| ChoiceOption {
                    value: l.name.clone(),
                    why: l
                        .config_file
                        .clone()
                        .map(|c| format!("Uses {c}"))
                        .unwrap_or_default(),
                    recommended: i == 0,
                })
                .collect(),
        });
    }
    let db_guess = databases
        .iter()
        .find(|d| d.contains(&name.replace('-', "_")) || d.ends_with("_dev"))
        .cloned()
        .or_else(|| databases.first().cloned());
    if databases.len() > 1 {
        choices.push(Choice {
            key: "db".into(),
            question: "Which local database should FloWW use for running and testing?".into(),
            options: databases
                .iter()
                .map(|d| ChoiceOption {
                    value: d.clone(),
                    why: if Some(d) == db_guess.as_ref() {
                        "Name matches this project".into()
                    } else {
                        String::new()
                    },
                    recommended: Some(d) == db_guess.as_ref(),
                })
                .collect(),
        });
    }

    let mut port = 8069u16;
    while used_ports.contains(&port) {
        port += 1;
    }

    Detection {
        repo: repo.to_string_lossy().into_owned(),
        name,
        is_git,
        preset: if modules.is_empty() {
            "custom".into()
        } else {
            "odoo".into()
        },
        module_count: modules.len(),
        suggested: Suggested {
            base_branch: "preprod".into(),
            db: db_guess.unwrap_or_default(),
            launch_config: launches.first().map(|l| l.name.clone()),
            http_port: port,
        },
        launches,
        databases,
        branches,
        default_branch,
        has_preprod,
        has_remote,
        initials,
        claude_installed,
        graphify_installed,
        checks,
        choices,
    }
}
