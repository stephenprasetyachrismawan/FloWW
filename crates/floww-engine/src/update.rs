//! Self-update from GitHub releases: check, download, verify, install,
//! remove old installer files. Settings, workspaces and the brain are never
//! touched: only files whose names match FloWW's own release files.

use std::cmp::Ordering;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REPO: &str = "stephenprasetyachrismawan/FloWW";
const SUMS: &str = "SHA256SUMS.txt";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// GitHub API root. Tests point it at a local server.
fn api_base() -> String {
    std::env::var("FLOWW_UPDATE_API").unwrap_or_else(|_| "https://api.github.com".into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Desktop app from the AppImage.
    AppImage,
    /// Desktop app installed from the .deb.
    Deb,
    /// floww-web binary from the release tarball.
    Web,
    /// Built from source: update with git.
    Source,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        serde_json::from_value(serde_json::Value::String(s.into())).ok()
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::AppImage => "app_image",
            Kind::Deb => "deb",
            Kind::Web => "web",
            Kind::Source => "source",
        }
    }
}

/// What is running and where it lives.
#[derive(Debug, Clone, PartialEq)]
pub struct Install {
    pub kind: Kind,
    /// The AppImage file, `/usr/bin/floww`, or the floww-web binary.
    pub target: PathBuf,
}

/// Work out how this copy of FloWW was installed.
pub fn detect_install(desktop: bool) -> Install {
    let exe = std::env::current_exe().unwrap_or_default();
    if desktop {
        if let Some(app_image) = std::env::var_os("APPIMAGE") {
            return Install {
                kind: Kind::AppImage,
                target: PathBuf::from(app_image),
            };
        }
        let from_deb = exe.starts_with("/usr/")
            && std::process::Command::new("dpkg")
                .arg("-S")
                .arg(&exe)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        return Install {
            kind: if from_deb { Kind::Deb } else { Kind::Source },
            target: exe,
        };
    }
    // cargo puts binaries in target/debug or target/release.
    let built_here = exe
        .parent()
        .filter(|p| p.ends_with("debug") || p.ends_with("release"))
        .and_then(Path::parent)
        .is_some_and(|p| p.ends_with("target"));
    Install {
        kind: if built_here { Kind::Source } else { Kind::Web },
        target: exe,
    }
}

/// Compare versions like 0.1.10 and 0.1.9 by number. Anything after `-` is ignored.
pub fn cmp_versions(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.trim()
            .trim_start_matches('v')
            .split('-')
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(a), parse(b));
    for i in 0..a.len().max(b.len()) {
        let o = a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0));
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

pub fn asset_name(kind: Kind, version: &str) -> Option<String> {
    match kind {
        Kind::AppImage => Some(format!("FloWW_{version}_amd64.AppImage")),
        Kind::Deb => Some(format!("FloWW_{version}_amd64.deb")),
        Kind::Web => Some(format!("floww-web-v{version}-linux-x86_64.tar.gz")),
        Kind::Source => None,
    }
}

/// Version in a FloWW release file or folder name, if it is one.
fn release_file_version(name: &str) -> Option<String> {
    let v = if let Some(rest) = name.strip_prefix("FloWW_") {
        rest.strip_suffix("_amd64.AppImage")
            .or_else(|| rest.strip_suffix("_amd64.deb"))?
    } else {
        let rest = name.strip_prefix("floww-web-v")?;
        rest.strip_suffix("-linux-x86_64.tar.gz")
            .or_else(|| rest.strip_suffix("-linux-x86_64"))?
    };
    let ok = !v.is_empty() && v.split('.').all(|p| p.parse::<u64>().is_ok());
    ok.then(|| v.to_string())
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache"))
        .join("floww")
        .join("updates")
}

/// Old FloWW installer files and folders (older than `new_version`) in the
/// download folders and next to the running copy.
pub fn old_files(install: &Install, new_version: &str) -> Vec<PathBuf> {
    let mut dirs = vec![home().join("Downloads"), home().join("Unduhan")];
    if let Some(parent) = install.target.parent() {
        dirs.push(parent.to_path_buf());
        // floww-web runs from inside its extracted folder; the tarball sits next to it.
        if let Some(grand) = parent.parent() {
            dirs.push(grand.to_path_buf());
        }
    }
    dirs.sort();
    dirs.dedup();
    let mut out = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(v) = release_file_version(&name) else {
                continue;
            };
            if cmp_versions(&v, new_version) == Ordering::Less {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    pub kind: Kind,
    pub release_url: String,
    pub notes: String,
    pub asset: Option<Asset>,
    #[serde(skip)]
    pub sums_url: Option<String>,
    pub old_files: Vec<String>,
    pub can_apply: bool,
    /// Why it cannot update by itself, and what to do instead.
    pub reason: Option<String>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

fn client(timeout: Option<Duration>) -> Result<reqwest::Client, String> {
    let mut b = reqwest::Client::builder()
        .user_agent(concat!("FloWW/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15));
    if let Some(t) = timeout {
        b = b.timeout(t);
    }
    b.build().map_err(|e| e.to_string())
}

fn writable_dir(dir: &Path) -> bool {
    let probe = dir.join(format!(".floww-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Ask GitHub for the latest release and compare it with this copy.
pub async fn check(install: &Install) -> Result<Check, String> {
    let url = format!("{}/repos/{REPO}/releases/latest", api_base());
    let res = client(Some(Duration::from_secs(20)))?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| {
            format!("Could not reach GitHub to check for updates ({e}). Check your internet connection and try again.")
        })?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("No FloWW release is published yet.".into());
    }
    if res.status() == reqwest::StatusCode::FORBIDDEN
        || res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        return Err(
            "GitHub is limiting update checks from your network right now. Try again in an hour."
                .into(),
        );
    }
    if !res.status().is_success() {
        return Err(format!(
            "GitHub answered {} when checking for updates. Try again later.",
            res.status()
        ));
    }
    let rel: GhRelease = res
        .json()
        .await
        .map_err(|e| format!("GitHub sent an answer FloWW could not read: {e}"))?;
    let latest = rel.tag_name.trim_start_matches('v').to_string();
    let current = current_version().to_string();
    let newer = cmp_versions(&latest, &current) == Ordering::Greater;
    let want = asset_name(install.kind, &latest);
    let asset = want.as_ref().and_then(|w| {
        rel.assets.iter().find(|a| &a.name == w).map(|a| Asset {
            name: a.name.clone(),
            url: a.browser_download_url.clone(),
            size: a.size,
        })
    });
    let sums_url = rel
        .assets
        .iter()
        .find(|a| a.name == SUMS)
        .map(|a| a.browser_download_url.clone());
    let reason = if !newer {
        None
    } else if install.kind == Kind::Source {
        Some("This copy was built from source. Update it with `git pull`, then build again.".into())
    } else if asset.is_none() {
        Some(format!(
            "Release {latest} has no {} file. Download it by hand from the release page.",
            want.unwrap_or_default()
        ))
    } else if sums_url.is_none() {
        Some(format!(
            "Release {latest} has no {SUMS}, so FloWW cannot check the download. Download it by hand from the release page."
        ))
    } else if install.kind == Kind::Deb && crate::proc::which("pkexec").is_none() {
        Some(
            "Installing the .deb needs pkexec (policykit-1), which is missing. Download the .deb and run `sudo apt install ./<file>.deb`."
                .into(),
        )
    } else if matches!(install.kind, Kind::AppImage | Kind::Web)
        && !install.target.parent().map(writable_dir).unwrap_or(false)
    {
        Some(format!(
            "FloWW cannot write to {}. Move FloWW to a folder you own, or download the new version by hand.",
            install.target.parent().unwrap_or(Path::new("/")).display()
        ))
    } else {
        None
    };
    let notes: String = rel.body.unwrap_or_default().chars().take(2000).collect();
    Ok(Check {
        can_apply: newer && reason.is_none(),
        old_files: if newer {
            old_files(install, &latest)
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        } else {
            Vec::new()
        },
        current,
        latest,
        newer,
        kind: install.kind,
        release_url: rel.html_url,
        notes,
        asset,
        sums_url,
        reason,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Check,
    Download,
    Verify,
    Install,
    Cleanup,
    Restart,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub stage: Stage,
    pub percent: Option<u8>,
    pub message: String,
}

impl Progress {
    pub fn new(stage: Stage, message: impl Into<String>) -> Self {
        Progress {
            stage,
            percent: None,
            message: message.into(),
        }
    }
}

fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let hash = it.next()?;
        let file = it.next()?.trim_start_matches('*');
        (file == name).then(|| hash.to_lowercase())
    })
}

fn mb(n: u64) -> String {
    format!("{:.1} MB", n as f64 / 1_000_000.0)
}

async fn download(
    url: &str,
    dest: &Path,
    size_hint: u64,
    on: &(dyn Fn(Progress) + Send + Sync),
) -> Result<String, String> {
    let fail = |e: String| {
        format!("Download failed: {e}. Check your connection and try the update again.")
    };
    let mut res = client(None)?
        .get(url)
        .send()
        .await
        .map_err(|e| fail(e.to_string()))?;
    if !res.status().is_success() {
        return Err(fail(format!("server answered {}", res.status())));
    }
    let total = res.content_length().unwrap_or(size_hint);
    let mut file = std::fs::File::create(dest).map_err(|e| fail(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut got: u64 = 0;
    let mut last = u8::MAX;
    while let Some(chunk) = res.chunk().await.map_err(|e| fail(e.to_string()))? {
        file.write_all(&chunk).map_err(|e| fail(e.to_string()))?;
        hasher.update(&chunk);
        got += chunk.len() as u64;
        let pct = (got * 100).checked_div(total).unwrap_or(0).min(100) as u8;
        if pct != last {
            last = pct;
            on(Progress {
                stage: Stage::Download,
                percent: (total > 0).then_some(pct),
                message: if total > 0 {
                    format!("Downloading {} of {}", mb(got), mb(total))
                } else {
                    format!("Downloading {}", mb(got))
                },
            });
        }
    }
    file.flush().map_err(|e| fail(e.to_string()))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Copy `src` next to `dest` and rename it over `dest`, so `dest` is never half written.
fn place(src: &Path, dest: &Path) -> Result<(), String> {
    let tmp = dest.with_extension("floww-new");
    std::fs::copy(src, &tmp).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not replace {}: {e}", dest.display())
    })
}

fn remove_path(p: &Path) -> std::io::Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}

/// Download, verify and install `check.latest`, then remove old installer files.
/// Returns the program to start afterwards.
pub async fn apply(
    install: &Install,
    check: &Check,
    on: &(dyn Fn(Progress) + Send + Sync),
) -> Result<PathBuf, String> {
    let asset = check.asset.as_ref().ok_or_else(|| {
        check
            .reason
            .clone()
            .unwrap_or_else(|| "Nothing to download.".into())
    })?;
    let sums_url = check
        .sums_url
        .as_ref()
        .ok_or_else(|| format!("Release {} has no {SUMS}.", check.latest))?;
    let dir = cache_dir();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;

    on(Progress::new(Stage::Download, "Starting download"));
    let sums = client(Some(Duration::from_secs(30)))?
        .get(sums_url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Could not download {SUMS}: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Could not read {SUMS}: {e}"))?;
    let want = expected_hash(&sums, &asset.name).ok_or_else(|| {
        format!(
            "{SUMS} does not list {}. The release looks broken; download it by hand.",
            asset.name
        )
    })?;
    let file = dir.join(&asset.name);
    let got = download(&asset.url, &file, asset.size, on).await?;

    on(Progress::new(Stage::Verify, "Checking the download"));
    if got != want {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!(
            "The download does not match {SUMS} (damaged or changed on the way). Nothing was installed. Try the update again."
        ));
    }

    on(Progress::new(Stage::Install, "Installing"));
    let mut keep: Vec<PathBuf> = Vec::new();
    let launch = match install.kind {
        Kind::AppImage => {
            let parent = install.target.parent().unwrap_or(Path::new("/"));
            let old_name = install
                .target
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Keep a custom name; rename a versioned one to the new version.
            let new_path = if release_file_version(&old_name).is_some() {
                parent.join(&asset.name)
            } else {
                install.target.clone()
            };
            place(&file, &new_path)?;
            if new_path != install.target {
                let _ = std::fs::remove_file(&install.target);
            }
            keep.push(new_path.clone());
            new_path
        }
        Kind::Deb => {
            on(Progress::new(
                Stage::Install,
                "Installing. Enter your password in the window that opens.",
            ));
            let out = tokio::process::Command::new("pkexec")
                .args(["apt-get", "install", "-y", "--allow-downgrades"])
                .arg(&file)
                .output()
                .await
                .map_err(|e| format!("Could not start pkexec: {e}"))?;
            if !out.status.success() {
                let code = out.status.code().unwrap_or(-1);
                let msg = if code == 126 || code == 127 {
                    "The password prompt was cancelled. Nothing changed. Try the update again."
                        .to_string()
                } else {
                    format!(
                        "apt-get could not install the update: {}. You can install it by hand: sudo apt install {}",
                        String::from_utf8_lossy(&out.stderr).trim(),
                        file.display()
                    )
                };
                return Err(msg);
            }
            install.target.clone()
        }
        Kind::Web => {
            let unpack = dir.join("unpack");
            std::fs::create_dir_all(&unpack).map_err(|e| e.to_string())?;
            crate::proc::run_ok(
                "tar",
                &[
                    "-xzf",
                    &file.to_string_lossy(),
                    "-C",
                    &unpack.to_string_lossy(),
                ],
                &dir,
                crate::proc::LONG,
            )
            .await
            .map_err(|e| format!("Could not unpack the update: {e}"))?;
            let folder = unpack.join(format!("floww-web-v{}-linux-x86_64", check.latest));
            let new_bin = folder.join("floww-web");
            if !new_bin.is_file() {
                return Err("The update has no floww-web program inside. The release looks broken; download it by hand.".into());
            }
            let parent = install.target.parent().unwrap_or(Path::new("/"));
            let in_release_folder = parent
                .file_name()
                .map(|n| release_file_version(&n.to_string_lossy()).is_some())
                .unwrap_or(false);
            let dest = if in_release_folder {
                // Running from the extracted folder: make the new one beside it.
                let new_dir = parent
                    .parent()
                    .unwrap_or(Path::new("/"))
                    .join(format!("floww-web-v{}-linux-x86_64", check.latest));
                std::fs::create_dir_all(&new_dir).map_err(|e| e.to_string())?;
                for entry in std::fs::read_dir(&folder)
                    .map_err(|e| e.to_string())?
                    .flatten()
                {
                    place(&entry.path(), &new_dir.join(entry.file_name()))?;
                }
                keep.push(new_dir.clone());
                new_dir.join("floww-web")
            } else {
                place(&new_bin, &install.target)?;
                install.target.clone()
            };
            keep.push(dest.clone());
            dest
        }
        Kind::Source => {
            return Err(check
                .reason
                .clone()
                .unwrap_or_else(|| "This copy cannot update itself.".into()));
        }
    };

    on(Progress::new(Stage::Cleanup, "Removing old files"));
    let mut removed = 0;
    for p in old_files(install, &check.latest) {
        // Never remove what was just installed, or a folder holding it.
        if keep.iter().any(|k| k.starts_with(&p)) {
            continue;
        }
        if remove_path(&p).is_ok() {
            removed += 1;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    on(Progress::new(
        Stage::Restart,
        format!(
            "Updated to {}. Removed {removed} old file{}. Starting FloWW…",
            check.latest,
            if removed == 1 { "" } else { "s" }
        ),
    ));
    Ok(launch)
}

/// Replace this process with `exe`, same arguments (without `--open`) and
/// the same access token, so open browser tabs keep working. Returns only on failure.
#[cfg(unix)]
pub fn restart_self(exe: &Path, token: &str) -> String {
    use std::os::unix::process::CommandExt;
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--open").collect();
    let err = std::process::Command::new(exe)
        .args(args)
        .env("FLOWW_TOKEN", token)
        .exec();
    format!(
        "Updated, but FloWW could not restart itself: {err}. Start {} again.",
        exe.display()
    )
}

/// Start `exe` on its own, so it keeps running after this process exits.
pub fn launch_detached(exe: &Path, args: &[&str]) -> Result<(), String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .env_remove("ARGV0")
        .env_remove("OWD");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start {}: {e}", exe.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert_eq!(cmp_versions("0.1.10", "0.1.9"), Ordering::Greater);
        assert_eq!(cmp_versions("v0.2.0", "0.1.99"), Ordering::Greater);
        assert_eq!(cmp_versions("0.1.1", "0.1.1"), Ordering::Equal);
        assert_eq!(cmp_versions("0.1", "0.1.0"), Ordering::Equal);
        assert_eq!(cmp_versions("0.1.0", "0.1.1"), Ordering::Less);
    }

    #[test]
    fn only_floww_release_names_count() {
        assert_eq!(
            release_file_version("FloWW_0.1.0_amd64.deb").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            release_file_version("FloWW_0.1.0_amd64.AppImage").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            release_file_version("floww-web-v0.1.0-linux-x86_64.tar.gz").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            release_file_version("floww-web-v0.1.0-linux-x86_64").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(release_file_version("FloWW_notes.deb"), None);
        assert_eq!(release_file_version("floww"), None);
        assert_eq!(release_file_version("FloWW_0.1.0_amd64.deb.bak"), None);
        assert_eq!(release_file_version("FloWW_x.1_amd64.deb"), None);
    }

    #[test]
    fn old_files_skip_new_versions_and_other_files() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path();
        for f in [
            "FloWW_0.1.0_amd64.deb",
            "FloWW_0.1.1_amd64.AppImage",
            "FloWW_0.1.2_amd64.AppImage",
            "floww-web-v0.1.0-linux-x86_64.tar.gz",
            "notes.txt",
        ] {
            std::fs::write(d.join(f), "").unwrap();
        }
        std::fs::create_dir(d.join("floww-web-v0.1.1-linux-x86_64")).unwrap();
        let install = Install {
            kind: Kind::AppImage,
            target: d.join("FloWW_0.1.1_amd64.AppImage"),
        };
        let names: Vec<String> = old_files(&install, "0.1.2")
            .iter()
            .filter(|p| p.starts_with(d))
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                "FloWW_0.1.0_amd64.deb",
                "FloWW_0.1.1_amd64.AppImage",
                "floww-web-v0.1.0-linux-x86_64.tar.gz",
                "floww-web-v0.1.1-linux-x86_64",
            ]
        );
    }

    #[test]
    fn sums_lookup() {
        let s = "abc123  FloWW_0.1.2_amd64.deb\nDEF456 *floww-web-v0.1.2-linux-x86_64.tar.gz\n";
        assert_eq!(
            expected_hash(s, "FloWW_0.1.2_amd64.deb").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            expected_hash(s, "floww-web-v0.1.2-linux-x86_64.tar.gz").as_deref(),
            Some("def456")
        );
        assert_eq!(expected_hash(s, "other"), None);
    }

    #[test]
    fn kind_names_round_trip() {
        for k in [Kind::AppImage, Kind::Deb, Kind::Web, Kind::Source] {
            assert_eq!(Kind::parse(k.as_str()), Some(k));
        }
    }
}
