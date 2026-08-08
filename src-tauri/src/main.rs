#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod crypto;

use commands::AppState;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            app.manage(AppState::open(&app_data)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::storage::storage_load,
            commands::storage::storage_save,
            commands::storage::storage_clear,
            commands::policy::policy_evaluate,
            commands::audit::audit_verify,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Local AI Assistant");
}
