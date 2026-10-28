//! Secrets live in the OS keyring, config files only hold references.
//!
//! - `keyring:<name>`: read with `secret-tool` (libsecret, GNOME/KDE keyring).
//! - `env:<NAME>`: read from an environment variable (CI, headless machines).

use std::path::Path;

use crate::proc;

pub async fn resolve(reference: &str) -> Result<String, String> {
    if let Some(name) = reference.strip_prefix("env:") {
        return std::env::var(name).map_err(|_| format!("environment variable {name} is not set"));
    }
    let Some(name) = reference.strip_prefix("keyring:") else {
        return Err(format!(
            "\"{reference}\" is not a secret reference. Use keyring:<name> or env:<NAME>."
        ));
    };
    // Fallback for machines without a keyring daemon.
    let env_name = format!(
        "FLOWW_SECRET_{}",
        name.to_uppercase().replace(['-', '.'], "_")
    );
    if let Ok(v) = std::env::var(&env_name) {
        return Ok(v);
    }
    let out = proc::run(
        "secret-tool",
        &["lookup", "service", "floww", "account", name],
        Path::new("."),
        proc::SHORT,
    )
    .await
    .map_err(|e| format!("{e}. Install libsecret-tools, or set {env_name}."))?;
    let value = out.stdout.trim_end_matches('\n').to_string();
    if out.ok && !value.is_empty() {
        Ok(value)
    } else {
        Err(format!(
            "no secret named \"{name}\" in the keyring. Save it in Settings, or set {env_name}."
        ))
    }
}

pub async fn store(name: &str, value: &str) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new("secret-tool")
        .args([
            "store",
            &format!("--label=FloWW {name}"),
            "service",
            "floww",
            "account",
            name,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| {
            "secret-tool is not installed. Install libsecret-tools to keep secrets in the keyring."
                .to_string()
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(value.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(format!("keyring:{name}"))
    } else {
        Err(format!(
            "could not save to the keyring: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
