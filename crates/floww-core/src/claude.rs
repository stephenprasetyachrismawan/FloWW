//! Building `claude -p` invocations and reading its stream-json output.
//!
//! FloWW runs every AI step through the Claude Code CLI. Auth is either the
//! user's existing Claude login or a 9router instance injected through env.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

use crate::config::AiAuth;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    /// Extra environment variables for the child process.
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct Request {
    pub prompt: String,
    pub cwd: PathBuf,
    pub model: Option<String>,
    /// Claude Code `--effort` (low, medium, high, xhigh, max). None: Claude Code's default.
    pub effort: Option<String>,
    pub resume_session: Option<String>,
    pub append_system_prompt: Option<String>,
    /// Let Claude edit and run commands without asking (develop and setup steps).
    pub bypass_permissions: bool,
}

/// 9router expects the bare base URL; Claude Code appends `/v1/messages`.
pub fn normalize_router_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string()
}

/// `router_token` is the resolved secret; it is only placed in the child env.
pub fn build(req: &Request, auth: &AiAuth, router_token: Option<&str>) -> Invocation {
    let mut args = vec![
        "-p".to_string(),
        req.prompt.clone(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
    ];
    if let Some(model) = &req.model {
        args.extend(["--model".into(), model.clone()]);
    }
    if let Some(effort) = &req.effort {
        args.extend(["--effort".into(), effort.clone()]);
    }
    if let Some(session) = &req.resume_session {
        args.extend(["--resume".into(), session.clone()]);
    }
    if let Some(extra) = &req.append_system_prompt {
        args.extend(["--append-system-prompt".into(), extra.clone()]);
    }
    if req.bypass_permissions {
        args.extend(["--permission-mode".into(), "bypassPermissions".into()]);
    }

    let mut env = Vec::new();
    if let AiAuth::Router { url, .. } = auth {
        env.push(("ANTHROPIC_BASE_URL".into(), normalize_router_url(url)));
        if let Some(token) = router_token {
            env.push(("ANTHROPIC_AUTH_TOKEN".into(), token.to_string()));
        }
    }

    Invocation {
        program: "claude".into(),
        args,
        env,
        cwd: req.cwd.clone(),
    }
}

/// What the UI needs from one line of stream-json output.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    SessionStarted {
        session_id: String,
        model: Option<String>,
    },
    AssistantText {
        text: String,
    },
    ToolUse {
        name: String,
        summary: String,
    },
    Finished {
        session_id: Option<String>,
        is_error: bool,
        result: Option<String>,
        cost_usd: Option<f64>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    Other,
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A short, human-readable summary of a tool call for the activity timeline.
fn tool_summary(name: &str, input: &Value) -> String {
    let pick = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("");
    match name {
        "Read" | "Edit" | "Write" => pick("file_path").to_string(),
        "Bash" => pick("command").lines().next().unwrap_or("").to_string(),
        "Grep" | "Glob" => pick("pattern").to_string(),
        _ => String::new(),
    }
}

/// Parse one line of `--output-format stream-json`. Unknown lines become `Other`.
pub fn parse_line(line: &str) -> Event {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Event::Other;
    };
    match v.get("type").and_then(Value::as_str) {
        Some("system") if v.get("subtype").and_then(Value::as_str) == Some("init") => {
            match str_field(&v, "session_id") {
                Some(session_id) => Event::SessionStarted {
                    session_id,
                    model: str_field(&v, "model"),
                },
                None => Event::Other,
            }
        }
        Some("assistant") => {
            let content = v.pointer("/message/content").and_then(Value::as_array);
            let Some(blocks) = content else {
                return Event::Other;
            };
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_use") => {
                        let name = str_field(block, "name").unwrap_or_default();
                        let summary =
                            tool_summary(&name, block.get("input").unwrap_or(&Value::Null));
                        return Event::ToolUse { name, summary };
                    }
                    Some("text") => {
                        if let Some(text) = str_field(block, "text") {
                            return Event::AssistantText { text };
                        }
                    }
                    _ => {}
                }
            }
            Event::Other
        }
        Some("result") => Event::Finished {
            session_id: str_field(&v, "session_id"),
            is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
            result: str_field(&v, "result"),
            cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
            input_tokens: v.pointer("/usage/input_tokens").and_then(Value::as_u64),
            output_tokens: v.pointer("/usage/output_tokens").and_then(Value::as_u64),
        },
        _ => Event::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> Request {
        Request {
            prompt: "Write the PRD".into(),
            cwd: "/repo".into(),
            model: Some("claude-sonnet-5-5".into()),
            ..Default::default()
        }
    }

    #[test]
    fn subscription_adds_no_env() {
        let inv = build(&req(), &AiAuth::Subscription, None);
        assert_eq!(inv.program, "claude");
        assert!(inv.env.is_empty());
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--model", "claude-sonnet-5-5"])
        );
        assert!(
            !inv.args.iter().any(|a| a == "--effort"),
            "no effort unless set"
        );
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--output-format", "stream-json"])
        );
    }

    #[test]
    fn router_sets_base_url_and_token() {
        let auth = AiAuth::Router {
            url: "http://localhost:20128/v1/".into(),
            token_ref: "keyring:floww-9router".into(),
        };
        let inv = build(&req(), &auth, Some("sk-test"));
        assert_eq!(
            inv.env,
            vec![
                (
                    "ANTHROPIC_BASE_URL".to_string(),
                    "http://localhost:20128".to_string()
                ),
                ("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-test".to_string()),
            ]
        );
    }

    #[test]
    fn resume_and_bypass_flags() {
        let mut r = req();
        r.resume_session = Some("abc".into());
        r.bypass_permissions = true;
        r.effort = Some("high".into());
        let inv = build(&r, &AiAuth::Subscription, None);
        assert!(inv.args.windows(2).any(|w| w == ["--resume", "abc"]));
        assert!(inv.args.windows(2).any(|w| w == ["--effort", "high"]));
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--permission-mode", "bypassPermissions"])
        );
    }

    #[test]
    fn parses_init_tool_and_result() {
        let init =
            r#"{"type":"system","subtype":"init","session_id":"s1","model":"claude-sonnet-5-5"}"#;
        assert_eq!(
            parse_line(init),
            Event::SessionStarted {
                session_id: "s1".into(),
                model: Some("claude-sonnet-5-5".into())
            }
        );

        let tool = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"sale_custom/models/sale.py"}}]}}"#;
        assert_eq!(
            parse_line(tool),
            Event::ToolUse {
                name: "Read".into(),
                summary: "sale_custom/models/sale.py".into()
            }
        );

        let result = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"s1","result":"done","total_cost_usd":0.12,"usage":{"input_tokens":1500,"output_tokens":300}}"#;
        assert_eq!(
            parse_line(result),
            Event::Finished {
                session_id: Some("s1".into()),
                is_error: false,
                result: Some("done".into()),
                cost_usd: Some(0.12),
                input_tokens: Some(1500),
                output_tokens: Some(300),
            }
        );
    }

    #[test]
    fn garbage_is_other() {
        assert_eq!(parse_line("not json"), Event::Other);
        assert_eq!(parse_line(r#"{"type":"user"}"#), Event::Other);
    }
}
