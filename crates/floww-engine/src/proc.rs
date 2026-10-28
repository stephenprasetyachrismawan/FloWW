//! Running external commands (git, psql, odoo-bin) with clear errors.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct Output {
    pub ok: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// Last lines of stderr (or stdout), for error messages.
    pub fn tail(&self, lines: usize) -> String {
        let src = if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        };
        let all: Vec<&str> = src.lines().collect();
        all[all.len().saturating_sub(lines)..].join("\n")
    }
}

/// Run a program and capture its output. Never goes through a shell.
pub async fn run(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> Result<Output, String> {
    run_env(program, args, cwd, &[], timeout).await
}

pub async fn run_env(
    program: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(String, String)],
    timeout: Duration,
) -> Result<Output, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("`{program}` is not installed or not on PATH")
        } else {
            format!("could not start `{program}`: {e}")
        }
    })?;
    let out = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("`{program}` did not finish within {} s", timeout.as_secs()))?
        .map_err(|e| format!("`{program}` failed: {e}"))?;
    Ok(Output {
        ok: out.status.success(),
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Like `run`, but a non-zero exit becomes an error with the output tail.
pub async fn run_ok(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let out = run(program, args, cwd, timeout).await?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(format!(
            "`{program} {}` failed:\n{}",
            args.join(" "),
            out.tail(12)
        ))
    }
}

pub const SHORT: Duration = Duration::from_secs(30);
pub const LONG: Duration = Duration::from_secs(60 * 30);

/// Is a program available on PATH?
pub fn which(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(program))
        .find(|p| p.is_file())
}
