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
            commands::browser::cleanup_stale_profiles(&app_data)?;
            app.manage(AppState::open(&app_data)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::storage::storage_load,
            commands::storage::storage_save,
            commands::storage::storage_clear,
            commands::storage::storage_list_projects,
            commands::storage::storage_create_project,
            commands::storage::storage_list_conversations,
            commands::storage::storage_create_conversation,
            commands::storage::storage_rename_conversation,
            commands::storage::storage_delete_conversation,
            commands::storage::storage_export_pdf,
            commands::storage::privacy_status,
            commands::task_control::task_stop_all,
            commands::policy::policy_evaluate,
            commands::policy::policy_get_access_mode,
            commands::policy::policy_set_access_mode,
            commands::audit::audit_verify,
            commands::audit::audit_list,
            commands::audit::audit_export,
            commands::audit::audit_record_interaction,
            commands::api::api_list_connectors,
            commands::api::api_save_connector,
            commands::api::api_delete_connector,
            commands::api::api_call,
            commands::ai_provider::ai_provider_list,
            commands::browser::browser_status,
            commands::browser::browser_open,
            commands::browser::browser_navigate,
            commands::browser::browser_fetch,
            commands::browser::browser_search,
            commands::browser::browser_snapshot,
            commands::browser::browser_act,
            commands::browser::browser_control,
            commands::browser::browser_close,
            commands::browser::browser_agent_status,
            commands::browser::browser_agent_start,
            commands::browser::browser_agent_pause,
            commands::browser::browser_quarantine_download,
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
            commands::pc::pc_media_control,
            commands::pc::pc_hand_control_set,
            commands::pc::pc_hand_pointer_move,
            commands::pc::pc_hand_pointer_action,
            commands::pc::pc_hand_scroll,
            commands::pc::pc_hand_window_action,
            commands::pc::pc_window_control,
            commands::screen::screen_analyze,
            commands::pc::pc_list_installed_applications,
            commands::pc::pc_open_installed_application,
            commands::profile::profile_load,
            commands::profile::profile_save,
            commands::profile::profile_clear,
            commands::profile::feedback_record,
            commands::profile::memory_status,
            commands::profile::memory_consolidate,
            commands::profile::memory_set_automatic,
            commands::profile::memory_rollback,
            commands::rag::rag_list_documents,
            commands::rag::rag_import_document,
            commands::rag::rag_search,
            commands::rag::rag_delete_document,
            commands::remote::remote_status,
            commands::remote::remote_pair,
            commands::remote::remote_unpair,
            commands::remote::remote_verify_envelope,
            commands::remote::remote_poll_once,
            commands::security::security_center_status,
            commands::security::security_incident_status,
            commands::security::security_remediate_threats,
            commands::security::security_quick_scan,
            commands::security::security_update_definitions,
            commands::sandbox::sandbox_status,
            commands::sandbox::evolution_detect_project_root,
            commands::sandbox::sandbox_install_runtime,
            commands::sandbox::evolution_record_proposal,
            commands::sandbox::evolution_list_proposals,
            commands::sandbox::evolution_generate_patch,
            commands::sandbox::evolution_inspect_patch,
            commands::sandbox::evolution_approve_patch,
            commands::sandbox::evolution_run_sandbox,
            commands::sandbox::evolution_promote_patch,
            commands::voice::voice_status,
            commands::voice::voice_transcribe,
            commands::voice::voice_speak,
            commands::voice::voice_stop_speaking,
            commands::workspace::workspace_list_grants,
            commands::workspace::workspace_add_grant,
            commands::workspace::workspace_remove_grant,
            commands::workspace::workspace_list_entries,
            commands::workspace::workspace_search,
            commands::workspace::workspace_read_text,
            commands::workspace::workspace_write_text,
            commands::workspace::workspace_create_directory,
            commands::workspace::workspace_copy,
            commands::workspace::workspace_move,
            commands::workspace::workspace_trash,
            commands::workspace::workspace_open,
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
