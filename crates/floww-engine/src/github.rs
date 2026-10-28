//! Pull requests on GitHub. Uses a token from the keyring (or env), or the
//! `gh` CLI when it is installed and signed in.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{git, proc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    pub state: String,
    pub merged: bool,
}

async fn slug(repo: &Path) -> Result<(String, String), String> {
    let url = git::remote_url(repo)
        .await
        .ok_or("this repo has no `origin` remote")?;
    git::github_slug(&url).ok_or_else(|| format!("origin ({url}) is not a GitHub repository"))
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("FloWW")
        .build()
        .map_err(|e| e.to_string())
}

pub async fn create(
    repo: &Path,
    token: Option<&str>,
    head: &str,
    base: &str,
    title: &str,
    body: &str,
) -> Result<PullRequest, String> {
    if let Some(token) = token {
        let (owner, name) = slug(repo).await?;
        let res = client()?
            .post(format!("https://api.github.com/repos/{owner}/{name}/pulls"))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "title": title, "body": body, "head": head, "base": base }))
            .send()
            .await
            .map_err(|e| format!("could not reach GitHub: {e}"))?;
        let status = res.status();
        let v: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            let msg = v
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(format!("GitHub refused the pull request ({status}): {msg}"));
        }
        return Ok(PullRequest {
            number: v["number"].as_u64().unwrap_or(0),
            url: v["html_url"].as_str().unwrap_or_default().to_string(),
            state: "open".into(),
            merged: false,
        });
    }
    if proc::which("gh").is_none() {
        return Err(
            "no GitHub token. Add one in Settings, or install and sign in to the `gh` CLI.".into(),
        );
    }
    let out = proc::run_ok(
        "gh",
        &[
            "pr", "create", "--head", head, "--base", base, "--title", title, "--body", body,
        ],
        repo,
        proc::LONG,
    )
    .await?;
    let url = out
        .lines()
        .rev()
        .find(|l| l.starts_with("https://"))
        .unwrap_or("")
        .trim()
        .to_string();
    let number = url
        .rsplit('/')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    Ok(PullRequest {
        number,
        url,
        state: "open".into(),
        merged: false,
    })
}

pub async fn status(repo: &Path, token: Option<&str>, number: u64) -> Result<PullRequest, String> {
    if let Some(token) = token {
        let (owner, name) = slug(repo).await?;
        let v: serde_json::Value = client()?
            .get(format!(
                "https://api.github.com/repos/{owner}/{name}/pulls/{number}"
            ))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| format!("could not reach GitHub: {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        return Ok(PullRequest {
            number,
            url: v["html_url"].as_str().unwrap_or_default().to_string(),
            state: v["state"].as_str().unwrap_or("open").to_string(),
            merged: v["merged"].as_bool().unwrap_or(false),
        });
    }
    let n = number.to_string();
    let out = proc::run_ok(
        "gh",
        &["pr", "view", &n, "--json", "url,state"],
        repo,
        proc::SHORT,
    )
    .await?;
    let v: serde_json::Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
    let state = v["state"].as_str().unwrap_or("OPEN").to_lowercase();
    Ok(PullRequest {
        number,
        url: v["url"].as_str().unwrap_or_default().to_string(),
        merged: state == "merged",
        state,
    })
}
