//! Folder checks and a small folder browser for picking a repository.
//! Paths always mean folders on the computer that runs FloWW.

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    pub is_git: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderListing {
    pub path: String,
    pub parent: Option<String>,
    pub home: String,
    pub is_git: bool,
    pub entries: Vec<FolderEntry>,
    /// True when the list was cut at [`MAX_ENTRIES`].
    pub truncated: bool,
}

const MAX_ENTRIES: usize = 500;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "your user".into())
}

fn host() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| std::fs::read_to_string("/etc/hostname"))
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "this computer".into())
}

/// Clean what a person typed or pasted: spaces, quotes, `~`, `file://`,
/// trailing slashes, `.` and `..`.
pub fn expand(input: &str) -> PathBuf {
    let mut s = input.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            s = &s[1..s.len() - 1];
        }
    }
    let s = s.strip_prefix("file://").unwrap_or(s);
    let raw = if s == "~" {
        home()
    } else if let Some(rest) = s.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(s)
    };
    let mut out = PathBuf::new();
    for c in raw.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn is_git(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn subfolders(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for e in std::fs::read_dir(dir)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // Follow symlinks: a linked folder is still a folder.
        if std::fs::metadata(e.path())
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            names.push(name);
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    Ok(names)
}

/// First folder on the way down from `/` that this user may not open.
fn blocked_at(path: &Path) -> Option<PathBuf> {
    let mut cur = PathBuf::new();
    for c in path.components() {
        cur.push(c);
        if let Err(e) = std::fs::read_dir(&cur) {
            if e.kind() == ErrorKind::PermissionDenied {
                return Some(cur);
            }
            return None;
        }
    }
    None
}

fn permission_error(path: &Path) -> String {
    let at = blocked_at(path).unwrap_or_else(|| path.to_path_buf());
    format!(
        "FloWW runs as {u} and is not allowed to open {at}. Give {u} access, for example \
         `sudo chmod o+rx {at}`, or start FloWW as the user who owns that folder.",
        u = user(),
        at = at.display()
    )
}

fn not_found_error(path: &Path) -> String {
    let mut ancestor = path.to_path_buf();
    let mut missing = None;
    while !ancestor.is_dir() {
        missing = ancestor
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        if !ancestor.pop() {
            break;
        }
    }
    let mut msg = format!(
        "{} does not exist on {} (the computer running FloWW).",
        path.display(),
        host()
    );
    let Some(missing) = missing else {
        return msg;
    };
    match subfolders(&ancestor) {
        Ok(names) => {
            let want = missing.to_lowercase();
            let close = names.iter().find(|n| n.to_lowercase() == want).or_else(|| {
                names.iter().find(|n| {
                    let n = n.to_lowercase();
                    n.starts_with(&want) || want.starts_with(&n)
                })
            });
            msg.push_str(&format!(
                " {} has no folder named \"{missing}\".",
                ancestor.display()
            ));
            if let Some(c) = close {
                msg.push_str(&format!(" Did you mean {}?", ancestor.join(c).display()));
            } else {
                if names.is_empty() {
                    msg.push_str(" It has no folders at all.");
                } else {
                    let shown: Vec<&str> = names.iter().take(8).map(String::as_str).collect();
                    let more = if names.len() > 8 { ", …" } else { "" };
                    msg.push_str(&format!(" Folders there: {}{more}.", shown.join(", ")));
                }
                msg.push_str(" Use Choose folder to pick it instead of typing.");
            }
        }
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            return permission_error(&ancestor);
        }
        Err(_) => {}
    }
    msg
}

/// Check that `input` is a folder FloWW can open. The error says what is
/// wrong and how to fix it.
pub fn check_folder(input: &str) -> Result<PathBuf, String> {
    if input.trim().is_empty() {
        return Err("Choose a folder first.".into());
    }
    let path = expand(input);
    if !path.is_absolute() {
        return Err(format!(
            "\"{}\" is not a full path. Use one that starts with /, for example {}.",
            input.trim(),
            home().join("work/acme-odoo").display()
        ));
    }
    match std::fs::metadata(&path) {
        Ok(m) if m.is_dir() => match std::fs::read_dir(&path) {
            Ok(_) => Ok(path),
            Err(e) if e.kind() == ErrorKind::PermissionDenied => Err(permission_error(&path)),
            Err(e) => Err(format!("Could not open {}: {e}.", path.display())),
        },
        Ok(_) => Err(format!(
            "{} is a file, not a folder. Choose the folder that holds it: {}.",
            path.display(),
            path.parent().unwrap_or(Path::new("/")).display()
        )),
        Err(e) if e.kind() == ErrorKind::NotFound => Err(not_found_error(&path)),
        Err(e) if e.kind() == ErrorKind::PermissionDenied => Err(permission_error(&path)),
        Err(e) => Err(format!("Could not open {}: {e}.", path.display())),
    }
}

/// Subfolders of `input` (your home folder when empty). Hidden folders are left out.
pub fn list(input: Option<&str>) -> Result<FolderListing, String> {
    let dir = match input.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => check_folder(p)?,
        None => home(),
    };
    let names = subfolders(&dir).map_err(|e| match e.kind() {
        ErrorKind::PermissionDenied => permission_error(&dir),
        _ => format!("Could not open {}: {e}.", dir.display()),
    })?;
    let visible: Vec<String> = names.into_iter().filter(|n| !n.starts_with('.')).collect();
    let truncated = visible.len() > MAX_ENTRIES;
    let entries = visible
        .into_iter()
        .take(MAX_ENTRIES)
        .map(|name| {
            let p = dir.join(&name);
            FolderEntry {
                is_git: is_git(&p),
                path: p.to_string_lossy().into_owned(),
                name,
            }
        })
        .collect();
    Ok(FolderListing {
        parent: dir.parent().map(|p| p.to_string_lossy().into_owned()),
        is_git: is_git(&dir),
        path: dir.to_string_lossy().into_owned(),
        home: home().to_string_lossy().into_owned(),
        entries,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_cleans_typed_paths() {
        assert_eq!(expand("  /a/b/  "), PathBuf::from("/a/b"));
        assert_eq!(expand("'/a/b c/'"), PathBuf::from("/a/b c"));
        assert_eq!(expand("\"/a/./x/../b\""), PathBuf::from("/a/b"));
        assert_eq!(expand("file:///srv/odoo"), PathBuf::from("/srv/odoo"));
        assert_eq!(expand("~/work"), home().join("work"));
    }

    #[test]
    fn trailing_slash_and_spaces_are_fine() {
        let t = tempfile::tempdir().unwrap();
        let p = format!(" {}/ \n", t.path().display());
        assert_eq!(check_folder(&p).unwrap(), t.path());
    }

    #[test]
    fn missing_folder_suggests_the_close_name() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("IndoCasa")).unwrap();
        let e = check_folder(&format!("{}/Indocasa/", t.path().display())).unwrap_err();
        assert!(e.contains("does not exist"), "{e}");
        assert!(e.contains("Did you mean"), "{e}");
        assert!(e.contains("IndoCasa"), "{e}");
    }

    #[test]
    fn missing_folder_lists_what_is_there() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("sale_custom")).unwrap();
        let e = check_folder(&format!("{}/nothing/deeper", t.path().display())).unwrap_err();
        assert!(e.contains("no folder named \"nothing\""), "{e}");
        assert!(e.contains("Folders there: sale_custom"), "{e}");
    }

    #[test]
    fn file_and_relative_paths_say_how_to_fix() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("x.txt");
        std::fs::write(&f, "").unwrap();
        let e = check_folder(f.to_str().unwrap()).unwrap_err();
        assert!(e.contains("is a file"), "{e}");
        let e = check_folder("work/odoo").unwrap_err();
        assert!(e.contains("not a full path"), "{e}");
        assert!(check_folder("  ").unwrap_err().contains("Choose a folder"));
    }

    #[test]
    fn list_shows_folders_and_marks_repos() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("b-repo/.git")).unwrap();
        std::fs::create_dir(t.path().join("A-plain")).unwrap();
        std::fs::create_dir(t.path().join(".hidden")).unwrap();
        std::fs::write(t.path().join("file.txt"), "").unwrap();
        let l = list(Some(t.path().to_str().unwrap())).unwrap();
        let names: Vec<_> = l
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.is_git))
            .collect();
        assert_eq!(names, vec![("A-plain", false), ("b-repo", true)]);
        assert_eq!(
            l.parent.as_deref(),
            t.path().parent().and_then(|p| p.to_str())
        );
        assert!(!l.is_git);
    }
}
