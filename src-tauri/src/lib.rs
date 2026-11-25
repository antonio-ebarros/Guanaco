mod engine;
mod hardware;
mod license;
mod llamafile;
mod ollama;
mod storage;
mod tools;
mod tts;
mod updater;

use tauri::{Manager, WindowEvent};

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("mailto:"))
    {
        return Err("URL inválida para abertura externa.".to_string());
    }

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", trimmed]);
        c
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(trimmed);
        c
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(trimmed);
        c
    };

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Não foi possível abrir o navegador: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ollama::AppState::default())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                ollama::cleanup_on_exit(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            hardware::get_system_info,
            ollama::start_ollama,
            ollama::install_ollama,
            ollama::list_installed_models,
            ollama::list_running_models,
            ollama::unload_model,
            ollama::delete_model,
            ollama::pull_model,
            ollama::cancel_chat,
            ollama::chat_stream,
            ollama::pick_gguf_file,
            ollama::create_custom_model,
            storage::load_conversations,
            storage::save_conversations,
            tts::get_tts_status,
            tts::install_piper_tts,
            tts::synthesize_piper,
            license::get_license_status,
            license::activate_license,
            license::deactivate_license,
            tools::pick_knowledge_folder,
            tools::scan_knowledge_folder,
            tools::export_conversation_markdown,
            llamafile::get_engine_config,
            llamafile::set_engine_config,
            llamafile::pick_llamafile,
            updater::check_app_update,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Guanaco");
}
