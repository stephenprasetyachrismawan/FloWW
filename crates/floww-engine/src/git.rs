//! Git operations FloWW needs. All through the `git` CLI, never a shell.

use std::path::Path;

use crate::proc::{self, LONG, SHORT};

pub async fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    proc::run_ok("git", args, cwd, LONG).await
}

pub async fn git_quick(cwd: &Path, args: &[&str]) -> Result<String, String> {
    proc::run_ok("git", args, cwd, SHORT).await
}

pub async fn is_repo(dir: &Path) -> bool {
    git_quick(dir, &["rev-parse", "--is-inside-work-tree"])
        .await
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

/// Top folder of the repository that contains `dir`.
pub async fn toplevel(dir: &Path) -> Option<std::path::PathBuf> {
    git_quick(dir, &["rev-parse", "--show-toplevel"])
        .await
        .ok()
        .map(|s| std::path::PathBuf::from(s.trim()))
        .filter(|p| p.is_dir())
}

pub async fn has_remote(repo: &Path) -> bool {
    git_quick(repo, &["remote"])
        .await
        .map(|s| s.lines().any(|l| l.trim() == "origin"))
        .unwrap_or(false)
}

pub async fn remote_url(repo: &Path) -> Option<String> {
    git_quick(repo, &["remote", "get-url", "origin"])
        .await
        .ok()
        .map(|s| s.trim().to_string())
}

pub async fn local_branch_exists(repo: &Path, branch: &str) -> bool {
    git_quick(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .await
    .is_ok()
}

pub async fn remote_branch_exists(repo: &Path, branch: &str) -> bool {
    git_quick(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/remotes/origin/{branch}"),
        ],
    )
    .await
    .is_ok()
}

pub async fn fetch(repo: &Path) -> Result<(), String> {
    if has_remote(repo).await {
        git(repo, &["fetch", "--prune", "origin"]).await?;
    }
    Ok(())
}

/// The ref to start new work from: `origin/<base>` if it exists, else local `<base>`.
pub async fn base_ref(repo: &Path, base: &str) -> Result<String, String> {
    if remote_branch_exists(repo, base).await {
        Ok(format!("origin/{base}"))
    } else if local_branch_exists(repo, base).await {
        Ok(base.to_string())
    } else {
        Err(format!(
            "branch \"{base}\" does not exist. Create it, or change \"New work starts from\" in Settings."
        ))
    }
}

pub async fn rev_parse(cwd: &Path, rev: &str) -> Result<String, String> {
    Ok(git_quick(cwd, &["rev-parse", rev])
        .await?
        .trim()
        .to_string())
}

pub async fn branches(repo: &Path) -> Vec<String> {
    git_quick(repo, &["branch", "--format=%(refname:short)"])
        .await
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub async fn default_branch(repo: &Path) -> Option<String> {
    if let Ok(s) = git_quick(
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .await
    {
        return s.trim().strip_prefix("origin/").map(str::to_string);
    }
    let all = branches(repo).await;
    ["main", "master"]
        .iter()
        .find(|b| all.iter().any(|x| x == *b))
        .map(|b| b.to_string())
}

/// New detached worktree at `start` (used for reading code before a branch exists).
pub async fn worktree_add_detached(repo: &Path, path: &Path, start: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let p = path.to_string_lossy();
    git(repo, &["worktree", "add", "--detach", &p, start])
        .await
        .map(|_| ())
}

pub async fn worktree_remove(repo: &Path, path: &Path) -> Result<(), String> {
    let p = path.to_string_lossy();
    git(repo, &["worktree", "remove", "--force", &p])
        .await
        .map(|_| ())
}

pub async fn switch_new_branch(worktree: &Path, branch: &str) -> Result<(), String> {
    git(worktree, &["switch", "-c", branch]).await.map(|_| ())
}

pub async fn changed_files(worktree: &Path, base: &str) -> Vec<String> {
    let mut files: Vec<String> = git_quick(worktree, &["diff", "--name-only", base])
        .await
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();
    // Untracked files count too.
    if let Ok(s) = git_quick(worktree, &["ls-files", "--others", "--exclude-standard"]).await {
        files.extend(s.lines().map(str::to_string));
    }
    files.sort();
    files.dedup();
    files
}

pub async fn diff(worktree: &Path, base: &str, max_bytes: usize) -> String {
    let mut d = git_quick(worktree, &["diff", base])
        .await
        .unwrap_or_default();
    if d.len() > max_bytes {
        let cut = (0..=max_bytes)
            .rev()
            .find(|i| d.is_char_boundary(*i))
            .unwrap_or(0);
        d.truncate(cut);
        d.push_str("\n… diff truncated");
    }
    d
}

pub async fn diff_stat(worktree: &Path, base: &str) -> String {
    git_quick(worktree, &["diff", "--stat", base])
        .await
        .unwrap_or_default()
}

/// Squash everything since `base` into one commit with the given message.
pub async fn squash_commit(worktree: &Path, base: &str, message: &str) -> Result<String, String> {
    git(worktree, &["add", "-A"]).await?;
    git(worktree, &["reset", "--soft", base]).await?;
    let staged = git_quick(worktree, &["diff", "--cached", "--name-only"]).await?;
    if staged.trim().is_empty() {
        return Err("there are no code changes on this branch to commit".into());
    }
    git(worktree, &["commit", "-m", message]).await?;
    rev_parse(worktree, "HEAD").await
}

pub async fn push(worktree: &Path, branch: &str) -> Result<(), String> {
    git(worktree, &["push", "-u", "origin", branch])
        .await
        .map(|_| ())
}

pub async fn user_name(repo: &Path) -> Option<String> {
    git_quick(repo, &["config", "user.name"])
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Add patterns to the repo's local exclude file (shared by all worktrees).
pub async fn exclude(repo: &Path, patterns: &[&str]) -> Result<(), String> {
    let common = git_quick(repo, &["rev-parse", "--git-common-dir"]).await?;
    let common = repo.join(common.trim());
    let file = common.join("info").join("exclude");
    std::fs::create_dir_all(file.parent().expect("has parent")).map_err(|e| e.to_string())?;
    let current = std::fs::read_to_string(&file).unwrap_or_default();
    let mut add = String::new();
    for p in patterns {
        if !current.lines().any(|l| l.trim() == *p) {
            add.push_str(p);
            add.push('\n');
        }
    }
    if !add.is_empty() {
        let mut text = current;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&add);
        std::fs::write(&file, text).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `owner/repo` from a GitHub remote URL.
pub fn github_slug(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.split("github.com/").nth(1))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    (!owner.is_empty() && !repo.is_empty()).then_some((owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_urls() {
        let want = Some(("acme".to_string(), "odoo".to_string()));
        assert_eq!(github_slug("git@github.com:acme/odoo.git"), want);
        assert_eq!(github_slug("https://github.com/acme/odoo"), want);
        assert_eq!(github_slug("https://x@github.com/acme/odoo.git/"), want);
        assert_eq!(github_slug("https://gitlab.com/acme/odoo"), None);
    }
}
