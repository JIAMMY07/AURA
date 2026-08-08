#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod crypto;

use commands::AppState;
use tauri::Manager;

fn main() {
    configure_private_webview();
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
            commands::api::api_list_connectors,
            commands::api::api_save_connector,
            commands::api::api_delete_connector,
            commands::api::api_call,
            commands::browser::browser_status,
            commands::browser::browser_open,
            commands::browser::browser_navigate,
            commands::browser::browser_fetch,
            commands::browser::browser_close,
            commands::creation3d::creation3d_status,
            commands::creation3d::creation3d_generate,
            commands::creation3d::creation3d_open_blender,
            commands::ollama::ollama_health,
            commands::ollama::ollama_list_models,
            commands::ollama::ollama_chat,
            commands::ollama::ollama_cancel,
            commands::ollama::guardrail_status,
            commands::pc::pc_system_snapshot,
            commands::pc::pc_list_processes,
            commands::pc::pc_open_application,
            commands::pc::pc_terminate_process,
            commands::pc::pc_lock_workstation,
            commands::profile::profile_load,
            commands::profile::profile_save,
            commands::profile::profile_clear,
            commands::profile::feedback_record,
            commands::rag::rag_list_documents,
            commands::rag::rag_import_document,
            commands::rag::rag_search,
            commands::rag::rag_delete_document,
            commands::security::security_center_status,
            commands::security::security_quick_scan,
            commands::security::security_update_definitions,
            commands::sandbox::sandbox_status,
            commands::sandbox::evolution_record_proposal,
            commands::sandbox::evolution_list_proposals,
            commands::sandbox::evolution_generate_patch,
            commands::sandbox::evolution_run_sandbox,
            commands::sandbox::evolution_promote_patch,
            commands::voice::voice_status,
            commands::voice::voice_transcribe,
            commands::voice::voice_speak,
            commands::voice::voice_stop_speaking,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Local AI Assistant");
}

fn configure_private_webview() {
    if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none() {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--disable-background-networking --disable-component-update --disable-domain-reliability --disable-sync --disable-features=msEdgeShoppingAssistant,msEdgeSidebarV2,msSmartScreenProtection --metrics-recording-only --no-first-run",
        );
    }
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let profile = std::path::PathBuf::from(local)
                .join("AURA")
                .join("webview2-private");
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", profile);
        }
    }
}
