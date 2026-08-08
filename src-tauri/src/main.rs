#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod crypto;

use commands::AppState;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            commands::voice::cleanup_stale_audio(&app_data)?;
            app.manage(AppState::open(&app_data)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::storage::storage_load,
            commands::storage::storage_save,
            commands::storage::storage_clear,
            commands::policy::policy_evaluate,
            commands::audit::audit_verify,
            commands::ollama::ollama_health,
            commands::ollama::ollama_list_models,
            commands::ollama::ollama_chat,
            commands::ollama::ollama_cancel,
            commands::pc::pc_system_snapshot,
            commands::pc::pc_list_processes,
            commands::pc::pc_open_application,
            commands::profile::profile_load,
            commands::profile::profile_save,
            commands::profile::profile_clear,
            commands::profile::feedback_record,
            commands::sandbox::sandbox_status,
            commands::sandbox::evolution_record_proposal,
            commands::voice::voice_status,
            commands::voice::voice_transcribe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Local AI Assistant");
}
