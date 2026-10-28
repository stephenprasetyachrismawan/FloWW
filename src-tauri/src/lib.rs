//! Desktop shell. Starts the same local server the web UI uses, on a random
//! loopback port, and gives the window its address and access token.
//!
//! Started with `--floww-update`, it is instead the small updater window that
//! installs a new version after the main app has closed, then opens it.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use floww_engine::update::{self, Install, Kind, Progress, Stage};
use serde::Serialize;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, Serialize, Default)]
struct ServerInfo {
    url: String,
    token: String,
    error: Option<String>,
}

struct ServerState(Mutex<ServerInfo>);

#[tauri::command]
fn server_info(state: tauri::State<'_, ServerState>) -> ServerInfo {
    state.0.lock().expect("server state").clone()
}

/// Close the app and hand over to the updater window.
#[tauri::command]
fn start_update(app: tauri::AppHandle) -> Result<(), String> {
    let install = update::detect_install(true);
    if install.kind == Kind::Source {
        return Err(
            "This copy was built from source. Update it with `git pull`, then build again.".into(),
        );
    }
    let target = install.target.to_string_lossy().into_owned();
    update::launch_detached(
        &install.target,
        &[
            "--floww-update",
            "--kind",
            install.kind.as_str(),
            "--target",
            &target,
        ],
    )?;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        app.exit(0);
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--floww-update") {
        return run_updater(&args);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ServerState(Mutex::new(ServerInfo::default())))
        .setup(|app| {
            let handle = app.handle().clone();
            let info = tauri::async_runtime::block_on(async {
                let token = floww_server::new_token();
                match floww_engine::Engine::open(None).await {
                    Ok(engine) => match floww_server::serve_with_origins(
                        engine,
                        0,
                        token.clone(),
                        floww_server::TAURI_ORIGINS
                            .iter()
                            .map(|o| o.to_string())
                            .collect(),
                    )
                    .await
                    {
                        Ok((addr, _task)) => ServerInfo {
                            url: format!("http://{addr}"),
                            token,
                            error: None,
                        },
                        Err(e) => ServerInfo {
                            error: Some(format!("could not start the local server: {e}")),
                            ..Default::default()
                        },
                    },
                    Err(e) => ServerInfo {
                        error: Some(format!("could not open FloWW: {e}")),
                        ..Default::default()
                    },
                }
            });
            *handle
                .state::<ServerState>()
                .0
                .lock()
                .expect("server state") = info;
            let main = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .ok_or("main window missing from tauri.conf.json")?;
            WebviewWindowBuilder::from_config(app.handle(), &main)?.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![server_info, start_update])
        .run(tauri::generate_context!())
        .expect("error while running FloWW");
}

/// What the updater window shows, and what to open if it fails.
struct UpdaterState {
    last: Mutex<Progress>,
    fallback: PathBuf,
}

#[tauri::command]
fn updater_state(state: tauri::State<'_, UpdaterState>) -> Progress {
    state.last.lock().expect("updater state").clone()
}

/// After a failed update: open the version that is still installed.
#[tauri::command]
fn updater_open_old(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdaterState>,
) -> Result<(), String> {
    update::launch_detached(&state.fallback, &[])?;
    app.exit(0);
    Ok(())
}

fn arg_after(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn run_updater(args: &[String]) {
    let kind = arg_after(args, "--kind").and_then(|k| Kind::parse(&k));
    let target = arg_after(args, "--target").map(PathBuf::from);
    let (Some(kind), Some(target)) = (kind, target) else {
        eprintln!("floww: --floww-update needs --kind and --target");
        std::process::exit(2);
    };
    let install = Install { kind, target };
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(UpdaterState {
            last: Mutex::new(Progress::new(Stage::Check, "Looking for the new version")),
            fallback: install.target.clone(),
        })
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "updater", WebviewUrl::App("index.html".into()))
                .title("Updating FloWW")
                .inner_size(460.0, 300.0)
                .resizable(false)
                .center()
                .initialization_script("window.__FLOWW_UPDATER__ = true;")
                .build()?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Give the old window time to close and this one time to load.
                tokio::time::sleep(Duration::from_millis(600)).await;
                let report = {
                    let handle = handle.clone();
                    move |p: Progress| {
                        if let Some(st) = handle.try_state::<UpdaterState>() {
                            *st.last.lock().expect("updater state") = p.clone();
                        }
                        let _ = handle.emit("update-progress", &p);
                    }
                };
                let result = match update::check(&install).await {
                    Err(e) => Err(e),
                    Ok(c) if !c.newer => {
                        report(Progress::new(
                            Stage::Restart,
                            format!(
                                "You already have the latest version ({}). Opening FloWW…",
                                c.current
                            ),
                        ));
                        Ok(install.target.clone())
                    }
                    Ok(c) if !c.can_apply => Err(c.reason.unwrap_or_default()),
                    Ok(c) => update::apply(&install, &c, &report).await,
                };
                match result {
                    Ok(exe) => match update::launch_detached(&exe, &[]) {
                        Ok(()) => {
                            tokio::time::sleep(Duration::from_millis(1500)).await;
                            handle.exit(0);
                        }
                        Err(e) => report(Progress::new(
                            Stage::Failed,
                            format!("Updated, but {e}. Open FloWW from the app menu."),
                        )),
                    },
                    Err(e) => report(Progress::new(Stage::Failed, e)),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![updater_state, updater_open_old])
        .run(tauri::generate_context!())
        .expect("error while running the FloWW updater");
}
