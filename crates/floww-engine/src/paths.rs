//! Where FloWW keeps things.

use std::path::{Path, PathBuf};

/// `~/.config/floww`, or `$FLOWW_HOME` (used by tests and portable installs).
pub fn app_home() -> PathBuf {
    if let Some(p) = std::env::var_os("FLOWW_HOME") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("floww")
}

pub fn app_config(home: &Path) -> PathBuf {
    home.join("app.toml")
}

pub struct WorkspacePaths {
    pub repo: PathBuf,
}

impl WorkspacePaths {
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self { repo: repo.into() }
    }
    pub fn floww(&self) -> PathBuf {
        self.repo.join(".floww")
    }
    pub fn runs(&self) -> PathBuf {
        self.floww().join("runs")
    }
    pub fn brain(&self) -> PathBuf {
        self.floww().join("brain")
    }
    pub fn worktrees(&self) -> PathBuf {
        self.floww().join("worktrees")
    }
    pub fn worktree(&self, prd: &str) -> PathBuf {
        self.worktrees().join(prd)
    }
    pub fn cache(&self) -> PathBuf {
        self.floww().join("cache")
    }
    pub fn logs(&self) -> PathBuf {
        self.floww().join("logs")
    }
}
