//! Desktop shell. Starts the same local server the web UI uses, on a random
//! loopback port, and gives the window its address and access token.

use std::sync::Mutex;

use serde::Serialize;
use tauri::Manager;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![server_info])
        .run(tauri::generate_context!())
        .expect("error while running FloWW");
}
