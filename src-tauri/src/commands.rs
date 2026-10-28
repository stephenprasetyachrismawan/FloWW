//! Tauri commands. Each one is a thin wrapper around `floww_core`.
//! Errors are returned as plain strings so the UI can show them as-is.

use std::path::PathBuf;

use floww_core::{config, odoo, pipeline, prd_id};
use serde::Serialize;

type CmdResult<T> = Result<T, String>;

fn err(e: floww_core::Error) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[derive(Serialize)]
pub struct StepInfo {
    number: u8,
    name: &'static str,
    phase: pipeline::Phase,
    actor: pipeline::Actor,
    needs_db_lock: bool,
}

#[tauri::command]
pub fn pipeline_steps() -> Vec<StepInfo> {
    pipeline::Step::ALL
        .iter()
        .map(|s| StepInfo {
            number: s.number(),
            name: s.name(),
            phase: s.phase(),
            actor: s.actor(),
            needs_db_lock: s.needs_db_lock(),
        })
        .collect()
}

#[tauri::command]
pub fn load_workspace(repo: PathBuf) -> CmdResult<config::EffectiveConfig> {
    config::load(&repo).map_err(err)
}

#[tauri::command]
pub fn launch_configs(repo: PathBuf) -> CmdResult<Vec<odoo::OdooLaunch>> {
    let path = repo.join(".vscode").join("launch.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    odoo::read_launch_configs(&path).map_err(err)
}

#[tauri::command]
pub fn next_prd_id(repo: PathBuf, initials: String) -> CmdResult<String> {
    prd_id::next_id(&repo.join(".floww").join("brain"), &initials).map_err(err)
}
