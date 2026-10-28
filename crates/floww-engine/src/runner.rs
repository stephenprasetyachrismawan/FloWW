//! Runs Claude Code (`claude -p ... --output-format stream-json`) and streams
//! its events back while it works.

use std::process::Stdio;
use std::time::Duration;

use floww_core::claude::{self, Event, Request};
use floww_core::config::AiAuth;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

#[derive(Debug, Clone, Default)]
pub struct AiRun {
    /// Final answer text (the `result` event, or the last assistant text).
    pub text: String,
    pub session_id: Option<String>,
    pub cost_usd: f64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

/// Program to run. Tests point this at a fake script.
pub fn claude_program() -> String {
    std::env::var("FLOWW_CLAUDE_BIN").unwrap_or_else(|_| "claude".into())
}

/// Run one Claude Code turn.
///
/// `on_event` sees every parsed event (tool calls, text) for the activity timeline.
/// Flipping `cancel` to `true` kills the process.
pub async fn run(
    req: &Request,
    auth: &AiAuth,
    router_token: Option<&str>,
    timeout: Duration,
    mut cancel: watch::Receiver<bool>,
    mut on_event: impl FnMut(&Event) + Send,
) -> Result<AiRun, String> {
    let inv = claude::build(req, auth, router_token);
    let program = claude_program();
    let mut cmd = Command::new(&program);
    cmd.args(&inv.args)
        .current_dir(&inv.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in &inv.env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "Claude Code (`claude`) is not installed or not on PATH. Install it from code.claude.com and sign in.".to_string()
        } else {
            format!("could not start Claude Code: {e}")
        }
    })?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let err_task = tokio::spawn(async move {
        let mut buf = String::new();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            if buf.len() < 8_000 {
                buf.push_str(&l);
                buf.push('\n');
            }
        }
        buf
    });

    let mut lines = BufReader::new(stdout).lines();
    let mut run = AiRun::default();
    let mut last_text = String::new();
    let mut finished_error: Option<String> = None;
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line.map_err(|e| e.to_string())? else { break };
                let ev = claude::parse_line(&line);
                match &ev {
                    Event::SessionStarted { session_id, .. } => run.session_id = Some(session_id.clone()),
                    Event::AssistantText { text } => last_text = text.clone(),
                    Event::Finished { session_id, is_error, result, cost_usd, input_tokens, output_tokens } => {
                        if let Some(s) = session_id { run.session_id = Some(s.clone()); }
                        run.text = result.clone().unwrap_or_default();
                        run.cost_usd = cost_usd.unwrap_or(0.0);
                        run.tokens_in = input_tokens.unwrap_or(0);
                        run.tokens_out = output_tokens.unwrap_or(0);
                        if *is_error {
                            finished_error = Some(result.clone().unwrap_or_else(|| "Claude Code reported an error".into()));
                        }
                    }
                    _ => {}
                }
                on_event(&ev);
            }
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    let _ = child.kill().await;
                    return Err("stopped".into());
                }
            }
            _ = &mut deadline => {
                let _ = child.kill().await;
                return Err(format!("Claude Code did not finish within {} minutes", timeout.as_secs() / 60));
            }
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    let stderr_text = err_task.await.unwrap_or_default();
    if let Some(e) = finished_error {
        return Err(e);
    }
    if !status.success() && run.text.is_empty() {
        let tail: Vec<&str> = stderr_text.lines().rev().take(8).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(format!(
            "Claude Code exited with {status}.\n{}",
            tail.join("\n")
        ));
    }
    if run.text.is_empty() {
        run.text = last_text;
    }
    Ok(run)
}
