//! Files FloWW puts in a PRD worktree. All are git-excluded, never committed.
//!
//! - `CLAUDE.local.md`: short instructions (commands, conventions, confirmed rules).
//!   Kept small on purpose: long generated context files cost more and help less.
//! - `.claude/settings.local.json`: a PostToolUse hook that checks every edit.
//! - `.claude/floww_check.py`: the check itself (Python stdlib only).
//! - `.floww-progress.md`: notes the AI keeps between short sessions.

use std::path::Path;

use crate::git;

pub const EXCLUDES: &[&str] = &[
    "CLAUDE.local.md",
    ".claude/settings.local.json",
    ".claude/floww_check.py",
    ".floww-progress.md",
];
pub const PROGRESS: &str = ".floww-progress.md";

pub struct Instructions<'a> {
    pub prd_id: &'a str,
    pub test_command: &'a str,
    pub upgrade_command: &'a str,
    pub commit_format: &'a str,
    pub rules: &'a [String],
    pub language: &'a str,
}

pub fn claude_local_md(i: &Instructions) -> String {
    let mut s = format!(
        "# FloWW work on {id}\n\n## Commands\n- Run module tests: `{test}`\n- Upgrade modules: `{up}`\n\n## Conventions\n- Work test-first: write a failing test, then make it pass.\n- Commit small local steps as `wip({id}): <what>`. FloWW squashes them later.\n- Keep notes for the next session in `{progress}`.\n- Final commit format: {fmt}.\n- Write user-facing text in {lang}.\n- Bump the module `version` in `__manifest__.py` when you add a migration.\n",
        id = i.prd_id,
        test = i.test_command,
        up = i.upgrade_command,
        progress = PROGRESS,
        fmt = i.commit_format,
        lang = i.language,
    );
    if !i.rules.is_empty() {
        s.push_str("\n## Business rules that must keep holding\n");
        for r in i.rules.iter().take(12) {
            s.push_str(&format!("- {r}\n"));
        }
    }
    s
}

pub const CHECK_PY: &str = r#"#!/usr/bin/env python3
"""FloWW post-edit check. Exit 2 sends the message back to Claude."""
import ast, json, os, sys
from xml.dom import minidom

def fail(msg):
    print(msg, file=sys.stderr)
    sys.exit(2)

data = json.load(sys.stdin)
path = (data.get("tool_input") or {}).get("file_path") or ""
if not path or not os.path.isfile(path):
    sys.exit(0)
with open(path, encoding="utf-8", errors="replace") as f:
    text = f.read()
for marker in ("<<<<<<< ", "\n=======\n", ">>>>>>> "):
    if marker in text:
        fail(f"{path} still has a git conflict marker. Resolve it before continuing.")
if path.endswith(".py"):
    try:
        tree = ast.parse(text, filename=path)
    except SyntaxError as e:
        fail(f"{path}:{e.lineno}: Python syntax error: {e.msg}. Fix the line before running tests.")
    if os.path.basename(path) == "__manifest__.py":
        try:
            manifest = ast.literal_eval(text)
        except Exception:
            fail(f"{path}: the manifest must be a plain dict literal.")
        base = os.path.dirname(path)
        for key in ("data", "demo"):
            for rel in manifest.get(key, []) or []:
                if not os.path.exists(os.path.join(base, rel)):
                    fail(f"{path}: '{key}' lists {rel}, which does not exist. Add the file or remove the entry.")
elif path.endswith(".xml"):
    try:
        minidom.parseString(text.encode("utf-8"))
    except Exception as e:
        fail(f"{path}: invalid XML: {e}. Odoo will refuse to load this view.")
sys.exit(0)
"#;

pub fn settings_local_json() -> String {
    serde_json::json!({
        "hooks": {
            "PostToolUse": [{
                "matcher": "Edit|Write|MultiEdit",
                "hooks": [{ "type": "command", "command": "python3 \"$CLAUDE_PROJECT_DIR/.claude/floww_check.py\"" }]
            }]
        }
    })
    .to_string()
}

/// Write the files and make sure git ignores them.
pub async fn install(
    repo: &Path,
    worktree: &Path,
    instructions: &Instructions<'_>,
) -> Result<(), String> {
    git::exclude(repo, EXCLUDES).await?;
    let w = |rel: &str, text: &str| -> Result<(), String> {
        let p = worktree.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, text).map_err(|e| format!("could not write {}: {e}", p.display()))
    };
    w("CLAUDE.local.md", &claude_local_md(instructions))?;
    w(".claude/floww_check.py", CHECK_PY)?;
    // Keep any settings.local.json the user already has.
    let settings = worktree.join(".claude/settings.local.json");
    if !settings.exists() {
        w(".claude/settings.local.json", &settings_local_json())?;
    }
    if !worktree.join(PROGRESS).exists() {
        w(
            PROGRESS,
            &format!("# Progress for {}\n\n", instructions.prd_id),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_stay_short() {
        let rules: Vec<String> = (0..30).map(|i| format!("rule {i}")).collect();
        let text = claude_local_md(&Instructions {
            prd_id: "SP-012",
            test_command: "odoo-bin --test-enable",
            upgrade_command: "odoo-bin -u x",
            commit_format: "[FIX] module: summary",
            rules: &rules,
            language: "Bahasa Indonesia",
        });
        assert!(text.lines().count() <= 60, "{} lines", text.lines().count());
        assert!(text.contains("rule 11") && !text.contains("rule 12"));
    }

    #[test]
    fn check_script_catches_bad_python() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("check.py");
        std::fs::write(&script, CHECK_PY).unwrap();
        let bad = dir.path().join("bad.py");
        std::fs::write(&bad, "def x(:\n").unwrap();
        let input = serde_json::json!({"tool_input": {"file_path": bad}}).to_string();
        let out = std::process::Command::new("python3")
            .arg(&script)
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(input.as_bytes())?;
                c.wait_with_output()
            })
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&out.stderr).contains("syntax error"));
    }
}
