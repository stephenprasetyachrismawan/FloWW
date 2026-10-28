use std::path::PathBuf;

/// Errors carry enough detail for the UI to tell the user what to fix.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {message}")]
    Toml { path: PathBuf, message: String },
    #[error("{path} is not valid JSON: {message}")]
    Json { path: PathBuf, message: String },
    #[error("launch.json has no configuration named \"{0}\"")]
    LaunchConfigNotFound(String),
    #[error("launch configuration \"{0}\" does not run odoo-bin")]
    NotAnOdooLaunch(String),
    #[error("cannot go from {from} to {to}: {reason}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
        reason: &'static str,
    },
    #[error("invalid value for {field}: {message}")]
    Invalid {
        field: &'static str,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
