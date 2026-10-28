mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::pipeline_steps,
            commands::load_workspace,
            commands::launch_configs,
            commands::next_prd_id,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FloWW");
}
