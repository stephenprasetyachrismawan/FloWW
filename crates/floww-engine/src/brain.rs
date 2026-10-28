//! The shared brain: an Obsidian vault on the orphan branch `floww/brain`,
//! checked out as a worktree at `.floww/brain`.
//!
//! Notes are generated from structured data (decisions, rules, changes), so
//! updates touch lines, not whole rewrites. Every note carries provenance.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;

use crate::git;
use crate::model::{GrillAnswer, Prd, Rule};
use crate::odoo_facts::ModuleFact;

pub const BRANCH: &str = "floww/brain";

pub async fn is_ready(brain: &Path) -> bool {
    brain.join(".git").exists()
}

/// Create or attach the brain worktree.
pub async fn init(repo: &Path, brain: &Path) -> Result<(), String> {
    if is_ready(brain).await {
        return Ok(());
    }
    if brain.exists()
        && std::fs::read_dir(brain)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(format!(
            "{} exists but is not the brain worktree. Move it away and try again.",
            brain.display()
        ));
    }
    let p = brain.to_string_lossy().into_owned();
    let _ = git::fetch(repo).await;
    if git::remote_branch_exists(repo, BRANCH).await {
        git::git(
            repo,
            &[
                "worktree",
                "add",
                "-B",
                BRANCH,
                &p,
                &format!("origin/{BRANCH}"),
            ],
        )
        .await?;
        return Ok(());
    }
    if git::local_branch_exists(repo, BRANCH).await {
        git::git(repo, &["worktree", "add", &p, BRANCH]).await?;
        return Ok(());
    }
    // New orphan branch with no history from the code branches.
    if git::git(repo, &["worktree", "add", "--orphan", "-b", BRANCH, &p])
        .await
        .is_err()
    {
        // Older git: detach, then start an orphan branch inside the worktree.
        git::git(repo, &["worktree", "add", "--detach", &p, "HEAD"]).await?;
        git::git(brain, &["checkout", "--orphan", BRANCH]).await?;
        git::git(brain, &["rm", "-rf", "--quiet", "."]).await?;
    }
    std::fs::write(
        brain.join("README.md"),
        "# FloWW brain\n\nShared notes for this project: decisions, business rules, flows, people, and code facts.\nOpen this folder as a vault in Obsidian.\n",
    )
    .map_err(|e| e.to_string())?;
    commit(brain, "brain: start").await?;
    Ok(())
}

/// Commit everything in the brain. No-op when nothing changed.
pub async fn commit(brain: &Path, message: &str) -> Result<bool, String> {
    git::git(brain, &["add", "-A"]).await?;
    let status = git::git_quick(brain, &["status", "--porcelain"]).await?;
    if status.trim().is_empty() {
        return Ok(false);
    }
    git::git(brain, &["commit", "-q", "-m", message]).await?;
    Ok(true)
}

/// Pull teammates' notes and push ours. Errors are reported, not fatal.
pub async fn sync(repo: &Path, brain: &Path) -> Result<(), String> {
    if !git::has_remote(repo).await {
        return Ok(());
    }
    if git::remote_branch_exists(repo, BRANCH).await {
        git::git(
            brain,
            &["pull", "--rebase", "--autostash", "origin", BRANCH],
        )
        .await?;
    }
    git::git(brain, &["push", "-u", "origin", BRANCH]).await?;
    Ok(())
}

pub fn write(brain: &Path, rel: &str, text: &str) -> Result<(), String> {
    let path = brain.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

// ---------- frontmatter ----------

pub fn frontmatter(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Some(rest) = text.strip_prefix("---\n") else {
        return map;
    };
    let Some(end) = rest.find("\n---") else {
        return map;
    };
    for line in rest[..end].lines() {
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
    map
}

fn body(text: &str) -> &str {
    text.strip_prefix("---\n")
        .and_then(|r| r.find("\n---").map(|e| r[e + 4..].trim_start_matches('\n')))
        .unwrap_or(text)
}

pub fn set_field(text: &str, key: &str, value: &str) -> String {
    let mut fm = frontmatter(text);
    fm.insert(key.into(), value.into());
    render(&fm, body(text))
}

fn render(fm: &BTreeMap<String, String>, body: &str) -> String {
    let mut s = String::from("---\n");
    for (k, v) in fm {
        s.push_str(&format!("{k}: {v}\n"));
    }
    s.push_str("---\n");
    s.push_str(body);
    s
}

fn provenance(author: &str, run: &str, model: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("author".into(), author.into());
    m.insert("run".into(), run.into());
    if !model.is_empty() {
        m.insert("model".into(), model.into());
    }
    m.insert("updated".into(), Utc::now().format("%Y-%m-%d").to_string());
    m
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

// ---------- notes generated from PRD data ----------

pub fn requirement_note(prd: &Prd) -> String {
    let mut fm = provenance(&prd.author, &prd.id, "");
    fm.insert("type".into(), "requirement".into());
    fm.insert("trust".into(), "fact".into());
    fm.insert("reporter".into(), prd.reporter.clone());
    render(
        &fm,
        &format!(
            "# {}\n\n{}\n\nPRD: [[prd/{}]]\n",
            prd.title, prd.description, prd.id
        ),
    )
}

pub fn prd_note(prd: &Prd, model: &str) -> String {
    let mut fm = provenance(&prd.author, &prd.id, model);
    fm.insert("type".into(), "prd".into());
    fm.insert("trust".into(), "confirmed".into());
    fm.insert("modules".into(), prd.modules.join(", "));
    render(&fm, prd.prd_markdown.as_deref().unwrap_or(""))
}

/// One row per grill answer, with a stable id so later updates touch one line.
pub fn decisions_note(prd: &Prd, answers: &[GrillAnswer], who: &str) -> String {
    let mut fm = provenance(who, &prd.id, "");
    fm.insert("type".into(), "decisions".into());
    fm.insert("trust".into(), "confirmed".into());
    let mut s = format!(
        "# Decisions for {}\n\nPRD: [[prd/{}]]\n\n| id | question | AI suggested | chosen | own answer |\n|---|---|---|---|---|\n",
        prd.title, prd.id
    );
    for (i, a) in answers.iter().enumerate() {
        let rec = a
            .question
            .options
            .iter()
            .find(|o| o.recommended)
            .map(|o| o.label.as_str())
            .unwrap_or("");
        s.push_str(&format!(
            "| D-{}-{} | {} | {} | {} | {} |\n",
            prd.id,
            i + 1,
            md_cell(&a.question.text),
            md_cell(rec),
            md_cell(&a.answer),
            if a.own { "yes" } else { "" }
        ));
    }
    render(&fm, &s)
}

pub fn rule_note(rule: &Rule, prd: &Prd, model: &str) -> String {
    let mut fm = provenance(&prd.author, &prd.id, model);
    fm.insert("type".into(), "rule".into());
    fm.insert("trust".into(), "draft".into());
    fm.insert("source".into(), format!("\"[[prd/{}]]\"", prd.id));
    fm.insert("enforced_in".into(), rule.enforced_in.clone());
    fm.insert(
        "valid_from".into(),
        Utc::now().format("%Y-%m-%d").to_string(),
    );
    fm.insert("used_ok".into(), "0".into());
    fm.insert("used_fail".into(), "0".into());
    render(&fm, &format!("# {}\n\n**Why:** {}\n", rule.title, rule.why))
}

pub fn changes_note(prd: &Prd, files: &[String], stat: &str) -> String {
    let mut fm = provenance(&prd.author, &prd.id, "");
    fm.insert("type".into(), "changes".into());
    fm.insert("trust".into(), "fact".into());
    fm.insert("branch".into(), prd.branch.clone().unwrap_or_default());
    let mut s = format!(
        "# Changes for {}\n\nPRD: [[prd/{}]]\n\n## Modules\n",
        prd.title, prd.id
    );
    for m in &prd.modules {
        s.push_str(&format!("- [[modules/{m}]]\n"));
    }
    s.push_str("\n## Files\n");
    for f in files {
        let mark = if f.contains("/migrations/") {
            " (migration)"
        } else {
            ""
        };
        s.push_str(&format!("- `{f}`{mark}\n"));
    }
    s.push_str(&format!("\n```\n{}\n```\n", stat.trim()));
    render(&fm, &s)
}

pub fn module_note(m: &ModuleFact) -> String {
    let mut fm = BTreeMap::new();
    fm.insert("type".into(), "module".into());
    fm.insert("trust".into(), "fact".into());
    fm.insert("version".into(), m.version.clone());
    fm.insert("path".into(), m.path.clone());
    let mut s = format!(
        "# {}\n\nDepends on: {}\n\n## Models\n",
        m.name,
        m.depends
            .iter()
            .map(|d| format!("[[modules/{d}]]"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for model in &m.models {
        s.push_str(&format!("\n### {}\n", model.name));
        if !model.inherits.is_empty() {
            s.push_str(&format!("inherits: {}\n", model.inherits.join(", ")));
        }
        s.push_str(&format!("file: `{}`\n", model.file));
        if !model.fields.is_empty() {
            s.push_str(&format!("fields: {}\n", model.fields.join(", ")));
        }
        if !model.methods.is_empty() {
            s.push_str(&format!("methods: {}\n", model.methods.join(", ")));
        }
    }
    render(&fm, &s)
}

pub fn opinion_note(who: &str, text: &str, source: &str) -> String {
    let mut fm = provenance(who, source, "");
    fm.insert("type".into(), "opinion".into());
    fm.insert("trust".into(), "confirmed".into());
    render(&fm, &format!("{text}\n"))
}

// ---------- reading ----------

#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub path: String,
    pub title: String,
    pub kind: String,
    pub trust: String,
    pub meta: BTreeMap<String, String>,
    pub excerpt: String,
}

fn title_of(text: &str, fallback: &str) -> String {
    body(text)
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

pub fn read_note(brain: &Path, rel: &str) -> Option<(Note, String)> {
    let text = std::fs::read_to_string(brain.join(rel)).ok()?;
    let meta = frontmatter(&text);
    let stem = Path::new(rel)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let b = body(&text);
    let excerpt: String = b
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    let note = Note {
        path: rel.to_string(),
        title: title_of(&text, &stem),
        kind: meta.get("type").cloned().unwrap_or_default(),
        trust: meta.get("trust").cloned().unwrap_or_default(),
        excerpt: excerpt.chars().take(240).collect(),
        meta,
    };
    Some((note, text))
}

pub fn list(brain: &Path, folder: &str) -> Vec<Note> {
    let dir = brain.join(folder);
    let mut files = Vec::new();
    collect_md(&dir, &mut files);
    let mut out: Vec<Note> = files
        .iter()
        .filter_map(|p| {
            let rel = p.strip_prefix(brain).ok()?.to_string_lossy().into_owned();
            read_note(brain, &rel).map(|(n, _)| n)
        })
        .collect();
    out.sort_by(|a, b| {
        b.meta
            .get("updated")
            .cmp(&a.meta.get("updated"))
            .then(a.path.cmp(&b.path))
    });
    out
}

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            collect_md(&p, out);
        } else if name.ends_with(".md") {
            out.push(p);
        }
    }
}

/// Notes that mention the query words, best first. Confirmed notes rank above drafts.
pub fn search(brain: &Path, query: &str, limit: usize) -> Vec<Note> {
    let words: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
        .filter(|w| w.len() >= 4)
        .map(str::to_string)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_md(brain, &mut files);
    let mut scored: Vec<(f32, Note)> = files
        .iter()
        .filter_map(|p| {
            let rel = p.strip_prefix(brain).ok()?.to_string_lossy().into_owned();
            let (note, text) = read_note(brain, &rel)?;
            let lower = text.to_lowercase();
            let hits = words.iter().filter(|w| lower.contains(w.as_str())).count();
            if hits == 0 {
                return None;
            }
            let trust_boost = match note.trust.as_str() {
                "confirmed" | "fact" => 1.0,
                _ => 0.5,
            };
            Some((hits as f32 * trust_boost, note))
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(limit).map(|(_, n)| n).collect()
}

/// Mark a draft note as confirmed by a person.
pub fn confirm(brain: &Path, rel: &str, who: &str) -> Result<(), String> {
    let path = brain.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let text = set_field(&text, "trust", "confirmed");
    let text = set_field(&text, "confirmed_by", who);
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

/// Count a rule as used in a run that passed or failed (usefulness score).
pub fn bump_usage(brain: &Path, rel: &str, ok: bool) -> Result<(), String> {
    let path = brain.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let key = if ok { "used_ok" } else { "used_fail" };
    let n: u32 = frontmatter(&text)
        .get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    std::fs::write(&path, set_field(&text, key, &(n + 1).to_string())).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_round_trip() {
        let t = "---\ntrust: draft\ntype: rule\n---\n# Title\n\nbody\n";
        assert_eq!(frontmatter(t).get("trust").unwrap(), "draft");
        let t2 = set_field(t, "trust", "confirmed");
        assert_eq!(frontmatter(&t2).get("trust").unwrap(), "confirmed");
        assert!(t2.contains("# Title"));
    }

    #[test]
    fn search_prefers_confirmed() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "rules/a.md",
            "---\ntrust: draft\n---\n# Invoice rule draft\ninvoice partial\n",
        )
        .unwrap();
        write(
            dir.path(),
            "rules/b.md",
            "---\ntrust: confirmed\n---\n# Invoice rule\ninvoice partial\n",
        )
        .unwrap();
        let hits = search(dir.path(), "invoice partial delivery", 5);
        assert_eq!(hits[0].path, "rules/b.md");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn usage_counter() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "rules/a.md", "---\nused_ok: 0\n---\n# R\n").unwrap();
        bump_usage(dir.path(), "rules/a.md", true).unwrap();
        bump_usage(dir.path(), "rules/a.md", true).unwrap();
        let (n, _) = read_note(dir.path(), "rules/a.md").unwrap();
        assert_eq!(n.meta.get("used_ok").unwrap(), "2");
    }
}
