//! Self-update against a fake GitHub release served locally.
//! Own test binary: it changes HOME and FLOWW_UPDATE_API for the process.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::response::IntoResponse;
use axum::routing::get;
use floww_engine::update::{self, Install, Kind, Stage};

/// Release files: name and bytes.
type Files = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
/// Progress seen: stage and percent.
type Seen = Arc<Mutex<Vec<(Stage, Option<u8>)>>>;

#[derive(Clone, Default)]
struct Fake {
    base: String,
    version: String,
    files: Files,
}

async fn latest(State(f): State<Fake>) -> impl IntoResponse {
    let assets: Vec<_> = f
        .files
        .lock()
        .unwrap()
        .iter()
        .map(|(n, d)| {
            serde_json::json!({
                "name": n,
                "size": d.len(),
                "browser_download_url": format!("{}/dl/{n}", f.base),
            })
        })
        .collect();
    axum::Json(serde_json::json!({
        "tag_name": format!("v{}", f.version),
        "html_url": format!("{}/release", f.base),
        "body": "Notes",
        "assets": assets,
    }))
}

async fn dl(State(f): State<Fake>, UrlPath(name): UrlPath<String>) -> impl IntoResponse {
    let files = f.files.lock().unwrap();
    match files.iter().find(|(n, _)| *n == name) {
        Some((_, d)) => (axum::http::StatusCode::OK, d.clone()).into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

fn sh(dir: &Path, cmd: &str) {
    let ok = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "{cmd}");
}

fn sums(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    use std::fmt::Write;
    let mut out = String::new();
    for (n, d) in files {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), d).unwrap();
        let o = std::process::Command::new("sha256sum")
            .arg("f")
            .current_dir(dir.path())
            .output()
            .unwrap();
        let hash = String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .next()
            .unwrap()
            .to_string();
        writeln!(out, "{hash}  {n}").unwrap();
    }
    out.into_bytes()
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

#[tokio::test]
async fn updates_web_and_appimage_and_removes_old_files() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().to_path_buf();
    let home = root.join("home");
    let downloads = home.join("Downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    for f in [
        "FloWW_0.0.1_amd64.deb",
        "FloWW_99.0.0_amd64.deb",
        "notes.txt",
    ] {
        std::fs::write(downloads.join(f), "x").unwrap();
    }
    let config = home.join(".config/floww/app.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "keep = true").unwrap();

    // Release 9.9.9: a floww-web tarball and an AppImage.
    let v = "9.9.9";
    let build = root.join("build");
    let folder = format!("floww-web-v{v}-linux-x86_64");
    std::fs::create_dir_all(build.join(&folder)).unwrap();
    std::fs::write(
        build.join(&folder).join("floww-web"),
        "#!/bin/sh\necho new\n",
    )
    .unwrap();
    std::fs::write(build.join(&folder).join("README.md"), "readme").unwrap();
    sh(&build, &format!("tar -czf web.tgz {folder}"));
    let mut files = vec![
        (
            format!("{folder}.tar.gz"),
            std::fs::read(build.join("web.tgz")).unwrap(),
        ),
        (
            format!("FloWW_{v}_amd64.AppImage"),
            b"new appimage".repeat(5000),
        ),
    ];
    let s = sums(&files);
    files.push(("SHA256SUMS.txt".into(), s));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake {
        base: base.clone(),
        version: v.into(),
        files: Arc::new(Mutex::new(files)),
    };
    let app = Router::new()
        .route("/repos/{o}/{r}/releases/latest", get(latest))
        .route("/dl/{name}", get(dl))
        .with_state(fake.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    // SAFETY: this test binary runs this one test; nothing else reads the environment concurrently.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CACHE_HOME", root.join("cache"));
        std::env::set_var("FLOWW_UPDATE_API", &base);
    }

    // floww-web running from its extracted release folder.
    let apps = root.join("apps");
    let old_dir = apps.join("floww-web-v0.1.1-linux-x86_64");
    std::fs::create_dir_all(&old_dir).unwrap();
    std::fs::write(old_dir.join("floww-web"), "old").unwrap();
    std::fs::write(apps.join("floww-web-v0.1.1-linux-x86_64.tar.gz"), "old").unwrap();
    let web = Install {
        kind: Kind::Web,
        target: old_dir.join("floww-web"),
    };
    let c = update::check(&web).await.unwrap();
    assert!(c.newer && c.can_apply, "{:?}", c.reason);
    assert_eq!(c.latest, v);
    assert!(
        c.old_files
            .iter()
            .any(|f| f.ends_with("FloWW_0.0.1_amd64.deb"))
    );
    assert!(
        c.old_files
            .iter()
            .any(|f| f.ends_with("floww-web-v0.1.1-linux-x86_64"))
    );
    assert!(
        !c.old_files
            .iter()
            .any(|f| f.contains("99.0.0") || f.ends_with("notes.txt"))
    );

    let seen: Seen = Default::default();
    let log = seen.clone();
    let on = move |p: update::Progress| log.lock().unwrap().push((p.stage, p.percent));
    let exe = update::apply(&web, &c, &on).await.unwrap();
    assert_eq!(exe, apps.join(&folder).join("floww-web"));
    assert!(read(&exe).contains("echo new"));
    assert_eq!(read(&apps.join(&folder).join("README.md")), "readme");
    assert!(!old_dir.exists(), "old floww-web folder removed");
    assert!(!apps.join("floww-web-v0.1.1-linux-x86_64.tar.gz").exists());
    assert!(!downloads.join("FloWW_0.0.1_amd64.deb").exists());
    assert!(
        downloads.join("FloWW_99.0.0_amd64.deb").exists(),
        "newer file kept"
    );
    assert!(downloads.join("notes.txt").exists(), "other files kept");
    assert_eq!(read(&config), "keep = true", "settings kept");
    let seen = seen.lock().unwrap().clone();
    assert!(
        seen.iter()
            .any(|(s, p)| *s == Stage::Download && *p == Some(100))
    );
    for s in [
        Stage::Verify,
        Stage::Install,
        Stage::Cleanup,
        Stage::Restart,
    ] {
        assert!(seen.iter().any(|(x, _)| *x == s), "{s:?} reported");
    }
    assert!(
        !root.join("cache/floww/updates").exists(),
        "download cache cleaned"
    );

    // AppImage with a versioned name: new file beside it, old one gone.
    let apps2 = root.join("apps2");
    std::fs::create_dir_all(&apps2).unwrap();
    let old_img = apps2.join("FloWW_0.1.1_amd64.AppImage");
    std::fs::write(&old_img, "old").unwrap();
    let img = Install {
        kind: Kind::AppImage,
        target: old_img.clone(),
    };
    let c = update::check(&img).await.unwrap();
    let exe = update::apply(&img, &c, &|_| {}).await.unwrap();
    assert_eq!(exe, apps2.join(format!("FloWW_{v}_amd64.AppImage")));
    assert!(!old_img.exists());
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&exe).unwrap().permissions().mode() & 0o111,
        0o111
    );

    // AppImage with a custom name is replaced in place.
    let custom: PathBuf = apps2.join("FloWW.AppImage");
    std::fs::write(&custom, "old").unwrap();
    let img = Install {
        kind: Kind::AppImage,
        target: custom.clone(),
    };
    let c = update::check(&img).await.unwrap();
    assert_eq!(update::apply(&img, &c, &|_| {}).await.unwrap(), custom);
    assert!(read(&custom).starts_with("new appimage"));

    // A download that does not match SHA256SUMS installs nothing.
    {
        let mut f = fake.files.lock().unwrap();
        let sums = f.iter_mut().find(|(n, _)| n == "SHA256SUMS.txt").unwrap();
        let text = String::from_utf8(sums.1.clone()).unwrap();
        sums.1 = text
            .lines()
            .map(|l| {
                format!(
                    "{}  {}\n",
                    "0".repeat(64),
                    l.split_whitespace().nth(1).unwrap()
                )
            })
            .collect::<String>()
            .into_bytes();
    }
    std::fs::write(&custom, "old").unwrap();
    let c = update::check(&img).await.unwrap();
    let e = update::apply(&img, &c, &|_| {}).await.unwrap_err();
    assert!(e.contains("does not match"), "{e}");
    assert_eq!(read(&custom), "old");

    // Same version: nothing newer.
    let same = Fake {
        version: update::current_version().into(),
        ..fake.clone()
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base2 = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/repos/{o}/{r}/releases/latest", get(latest))
        .with_state(same);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    unsafe { std::env::set_var("FLOWW_UPDATE_API", &base2) };
    let c = update::check(&web).await.unwrap();
    assert!(!c.newer && !c.can_apply && c.old_files.is_empty());
}
