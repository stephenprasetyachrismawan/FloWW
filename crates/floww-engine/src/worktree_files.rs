//! The one file FloWW puts in a PRD worktree: `CLAUDE.local.md`, with the
//! Odoo commands and team conventions Claude Code cannot know by itself.
//! Git-excluded, never committed. Everything else is Claude Code's own.

use std::path::Path;

use crate::git;

/// Also lists files older versions wrote, so old worktrees stay clean.
pub const EXCLUDES: &[&str] = &[
    "CLAUDE.local.md",
    ".claude/settings.local.json",
    ".claude/floww_check.py",
    ".floww-progress.md",
];

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
        "# FloWW work on {id}\n\n## Commands\n- Run module tests: `{test}`\n- Upgrade modules: `{up}`\n\n## Conventions\n- Commit small local steps as `wip({id}): <what>`. FloWW squashes them later.\n- Final commit format: {fmt}.\n- Write user-facing text in {lang}.\n- Bump the module `version` in `__manifest__.py` when you add a migration.\n",
        id = i.prd_id,
        test = i.test_command,
        up = i.upgrade_command,
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

/// Write `CLAUDE.local.md` and make sure git ignores it.
pub async fn install(
    repo: &Path,
    worktree: &Path,
    instructions: &Instructions<'_>,
) -> Result<(), String> {
    git::exclude(repo, EXCLUDES).await?;
    let p = worktree.join("CLAUDE.local.md");
    std::fs::write(&p, claude_local_md(instructions))
        .map_err(|e| format!("could not write {}: {e}", p.display()))
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
}
