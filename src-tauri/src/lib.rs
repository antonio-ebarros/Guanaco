mod engine;
mod hardware;
mod ollama;
mod storage;
mod tts;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ollama::AppState::default())
        .manage(tts::TtsState::default())
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
            storage::load_conversations,
            storage::save_conversations,
            tts::get_tts_status,
            tts::install_kokoro_tts,
            tts::unload_kokoro_tts,
            tts::synthesize_kokoro
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Guanaco");
}
