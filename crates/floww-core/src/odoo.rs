//! Odoo preset: read `.vscode/launch.json`, build run/upgrade/test commands,
//! and find which modules a change touches.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

/// One entry from launch.json that runs odoo-bin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OdooLaunch {
    pub name: String,
    /// Python interpreter, if launch.json names one.
    pub python: Option<String>,
    pub odoo_bin: String,
    /// Arguments from launch.json, minus the ones FloWW sets itself.
    pub base_args: Vec<String>,
    pub config_file: Option<String>,
}

/// Remove `//` and `/* */` comments and trailing commas (VS Code allows them).
pub fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    remove_trailing_commas(&out)
}

fn remove_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
            if !matches!(next, Some('}') | Some(']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// Flags FloWW controls per run, so they are dropped from launch.json args.
const MANAGED_FLAGS: &[&str] = &[
    "-d",
    "--database",
    "--http-port",
    "-p",
    "-u",
    "--update",
    "-i",
    "--init",
];

fn strip_managed(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip_next = false;
    for a in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if MANAGED_FLAGS.contains(&a.as_str()) {
            skip_next = true;
            continue;
        }
        let flag = a.split('=').next().unwrap_or(a);
        if a.contains('=') && MANAGED_FLAGS.contains(&flag) {
            continue;
        }
        out.push(a.clone());
    }
    out
}

fn config_file(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "-c" || a == "--config" {
            return it.next().cloned();
        }
        if let Some(v) = a.strip_prefix("--config=") {
            return Some(v.to_string());
        }
    }
    None
}

fn to_launch(cfg: &Value) -> Option<OdooLaunch> {
    let name = cfg.get("name")?.as_str()?.to_string();
    let program = cfg.get("program").and_then(Value::as_str).unwrap_or("");
    let args: Vec<String> = cfg
        .get("args")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let is_odoo = |s: &str| s.ends_with("odoo-bin") || s.ends_with("odoo-bin\"");
    let odoo_bin = if is_odoo(program) {
        program.to_string()
    } else {
        args.iter().find(|a| is_odoo(a))?.clone()
    };
    let rest: Vec<String> = args.iter().filter(|a| **a != odoo_bin).cloned().collect();
    Some(OdooLaunch {
        name,
        python: cfg
            .get("python")
            .and_then(Value::as_str)
            .map(str::to_string),
        config_file: config_file(&rest),
        base_args: strip_managed(&rest),
        odoo_bin,
    })
}

/// All odoo-bin configurations in a launch.json file.
pub fn read_launch_configs(path: &Path) -> Result<Vec<OdooLaunch>> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let v: Value = serde_json::from_str(&strip_jsonc(&text)).map_err(|e| Error::Json {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    Ok(v.get("configurations")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(to_launch).collect())
        .unwrap_or_default())
}

pub fn find_launch(configs: &[OdooLaunch], name: &str) -> Result<OdooLaunch> {
    configs
        .iter()
        .find(|c| c.name == name)
        .cloned()
        .ok_or_else(|| Error::LaunchConfigNotFound(name.to_string()))
}

/// `${workspaceFolder}` resolves to the worktree the PRD runs in.
fn resolve(arg: &str, workspace: &Path) -> String {
    arg.replace("${workspaceFolder}", &workspace.to_string_lossy())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

pub enum Mode<'a> {
    Run,
    Upgrade(&'a [String]),
    Test(&'a [String]),
}

pub fn command(
    launch: &OdooLaunch,
    workspace: &Path,
    db: &str,
    http_port: u16,
    mode: Mode,
) -> Command {
    let odoo_bin = resolve(&launch.odoo_bin, workspace);
    let (program, mut args) = match &launch.python {
        Some(py) => (resolve(py, workspace), vec![odoo_bin]),
        None => (odoo_bin, Vec::new()),
    };
    args.extend(launch.base_args.iter().map(|a| resolve(a, workspace)));
    args.extend(["-d".into(), db.to_string()]);
    match mode {
        Mode::Run => args.extend(["--http-port".into(), http_port.to_string()]),
        Mode::Upgrade(mods) => {
            args.extend(["-u".into(), mods.join(","), "--stop-after-init".into()])
        }
        Mode::Test(mods) => {
            let tags = mods
                .iter()
                .map(|m| format!("/{m}"))
                .collect::<Vec<_>>()
                .join(",");
            args.extend([
                "--test-enable".into(),
                "--test-tags".into(),
                tags,
                "--stop-after-init".into(),
            ]);
        }
    }
    Command { program, args }
}

/// Modules touched by a list of changed paths (relative to `repo`).
///
/// A module is the nearest parent folder with a `__manifest__.py`.
pub fn modules_touched(repo: &Path, changed: &[PathBuf]) -> Vec<String> {
    let mut found = BTreeSet::new();
    for path in changed {
        let mut dir = repo.join(path);
        if !dir.is_dir() {
            dir.pop();
        }
        while dir.starts_with(repo) && dir != repo {
            if dir.join("__manifest__.py").is_file() {
                if let Some(name) = dir.file_name() {
                    found.insert(name.to_string_lossy().into_owned());
                }
                break;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    found.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAUNCH: &str = r#"{
        // VS Code launch file
        "version": "0.2.0",
        "configurations": [
            {
                "name": "Odoo 17 (acme)",
                "type": "debugpy",
                "request": "launch",
                "python": "${workspaceFolder}/.venv/bin/python",
                "program": "/opt/odoo/17.0/odoo-bin",
                "args": ["-c", "odoo17.conf", "-d", "old_db", "--dev=all", "-u", "sale_custom"],
            },
            /* not odoo */
            { "name": "Pytest", "program": "pytest", "args": [] },
        ],
    }"#;

    fn write_launch(dir: &Path) -> PathBuf {
        let p = dir.join(".vscode/launch.json");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, LAUNCH).unwrap();
        p
    }

    #[test]
    fn reads_odoo_configs_from_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        let configs = read_launch_configs(&write_launch(dir.path())).unwrap();
        assert_eq!(configs.len(), 1);
        let c = &configs[0];
        assert_eq!(c.name, "Odoo 17 (acme)");
        assert_eq!(c.odoo_bin, "/opt/odoo/17.0/odoo-bin");
        assert_eq!(c.config_file.as_deref(), Some("odoo17.conf"));
        assert_eq!(c.base_args, vec!["-c", "odoo17.conf", "--dev=all"]);
    }

    #[test]
    fn builds_upgrade_and_test_commands() {
        let dir = tempfile::tempdir().unwrap();
        let configs = read_launch_configs(&write_launch(dir.path())).unwrap();
        let launch = find_launch(&configs, "Odoo 17 (acme)").unwrap();
        let wt = Path::new("/work/wt/SP-012");
        let mods = vec!["sale_custom".to_string(), "account_custom".to_string()];

        let up = command(&launch, wt, "acme_dev", 8069, Mode::Upgrade(&mods));
        assert_eq!(up.program, "/work/wt/SP-012/.venv/bin/python");
        assert_eq!(
            up.args,
            vec![
                "/opt/odoo/17.0/odoo-bin",
                "-c",
                "odoo17.conf",
                "--dev=all",
                "-d",
                "acme_dev",
                "-u",
                "sale_custom,account_custom",
                "--stop-after-init"
            ]
        );

        let t = command(&launch, wt, "acme_dev", 8069, Mode::Test(&mods));
        assert!(
            t.args
                .windows(2)
                .any(|w| w == ["--test-tags", "/sale_custom,/account_custom"])
        );

        let run = command(&launch, wt, "acme_dev", 8070, Mode::Run);
        assert!(run.args.windows(2).any(|w| w == ["--http-port", "8070"]));
    }

    #[test]
    fn missing_launch_name_is_clear() {
        let err = find_launch(&[], "Odoo 18").unwrap_err().to_string();
        assert!(err.contains("Odoo 18"));
    }

    #[test]
    fn finds_touched_modules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for m in ["addons/sale_custom", "addons/account_custom"] {
            std::fs::create_dir_all(root.join(m).join("models")).unwrap();
            std::fs::write(root.join(m).join("__manifest__.py"), "{}").unwrap();
        }
        let changed = vec![
            PathBuf::from("addons/sale_custom/models/sale_order_line.py"),
            PathBuf::from("addons/sale_custom/__manifest__.py"),
            PathBuf::from("README.md"),
        ];
        assert_eq!(modules_touched(root, &changed), vec!["sale_custom"]);
    }

    #[test]
    fn jsonc_keeps_slashes_inside_strings() {
        let s = strip_jsonc(r#"{"url": "http://x//y", /* c */ "a": [1,2,],}"#);
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["url"], "http://x//y");
        assert_eq!(v["a"].as_array().unwrap().len(), 2);
    }
}
