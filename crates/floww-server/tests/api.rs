//! The local API's security rules and a basic round trip.

use floww_engine::Engine;

async fn start() -> (String, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(Some(dir.path().join("home"))).await.unwrap();
    let token = floww_server::new_token();
    let (addr, _h) = floww_server::serve(engine, 0, token.clone()).await.unwrap();
    (format!("http://{addr}"), token, dir)
}

#[tokio::test]
async fn api_needs_the_token() {
    let (base, token, _d) = start().await;
    let c = reqwest::Client::new();
    let r = c.get(format!("{base}/api/app")).send().await.unwrap();
    assert_eq!(r.status(), 401);
    let r = c
        .get(format!("{base}/api/app"))
        .bearer_auth("wrong")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let r = c
        .get(format!("{base}/api/app"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = r.json().await.unwrap();
    assert_eq!(v["ai_ready"], false);
}

#[tokio::test]
async fn token_in_query_only_for_events() {
    let (base, token, _d) = start().await;
    let c = reqwest::Client::new();
    let r = c
        .get(format!("{base}/api/app?token={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        401,
        "query token must not work for normal API calls"
    );
    let r = c
        .get(format!("{base}/api/events?token={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "text/event-stream");
}

#[tokio::test]
async fn foreign_host_is_rejected() {
    let (base, token, _d) = start().await;
    let r = reqwest::Client::new()
        .get(format!("{base}/api/app"))
        .header("Host", "evil.example:80")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn set_ai_and_errors_are_json() {
    let (base, token, dir) = start().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/api/ai"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "mode": "subscription", "model": "claude-sonnet-5-5" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = c
        .get(format!("{base}/api/app"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["ai_ready"], true);
    assert_eq!(v["default_model"], "claude-sonnet-5-5");

    let r = c
        .post(format!("{base}/api/detect"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "path": "/no/such/folder" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    let v: serde_json::Value = r.json().await.unwrap();
    let e = v["error"].as_str().unwrap();
    assert!(e.contains("/no/such/folder does not exist"), "{e}");
    assert!(e.contains("/ has no folder named \"no\""), "{e}");

    // The folder picker lists subfolders and marks git repos.
    std::fs::create_dir_all(dir.path().join("pick/repo/.git")).unwrap();
    let v: serde_json::Value = c
        .get(format!("{base}/api/folders"))
        .query(&[("path", dir.path().join("pick").to_str().unwrap())])
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["entries"][0]["name"], "repo");
    assert_eq!(v["entries"][0]["is_git"], true);
    let r = c.get(format!("{base}/api/folders")).send().await.unwrap();
    assert_eq!(r.status(), 401, "folder list needs the token");

    // Branch name preview: default template, then a broken one.
    let v: serde_json::Value = c
        .get(format!("{base}/api/branch-preview"))
        .query(&[("template", "dev/{ddmmyyyy}_{slug}"), ("initials", "sp")])
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let name = v["name"].as_str().unwrap();
    assert!(
        name.starts_with("dev/") && name.ends_with("_invoice-dobel-saat"),
        "{name}"
    );
    assert_eq!(name.len(), "dev/".len() + 8 + "_invoice-dobel-saat".len());
    let r = c
        .get(format!("{base}/api/branch-preview"))
        .query(&[("template", "dev/{date}")])
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn cors_only_for_the_desktop_webview() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(Some(dir.path().join("home"))).await.unwrap();
    let token = floww_server::new_token();
    let origins = floww_server::TAURI_ORIGINS
        .iter()
        .map(|o| o.to_string())
        .collect();
    let (addr, _h) = floww_server::serve_with_origins(engine, 0, token.clone(), origins)
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let c = reqwest::Client::new();
    let r = c
        .get(format!("{base}/api/app"))
        .header("Origin", "tauri://localhost")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.headers()["access-control-allow-origin"],
        "tauri://localhost"
    );
    let r = c
        .get(format!("{base}/api/app"))
        .header("Origin", "https://evil.example")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(r.headers().get("access-control-allow-origin").is_none());
    let pre = c
        .request(reqwest::Method::OPTIONS, format!("{base}/api/app"))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert!(pre.headers().get("access-control-allow-origin").is_none());
}
