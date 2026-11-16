use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

const OLLAMA_URL: &str = "http://127.0.0.1:11434";

struct AppState {
    chat_cancel: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    ollama_child: Mutex<Option<Child>>,
    active_model: Mutex<Option<String>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            chat_cancel: Mutex::new(None),
            ollama_child: Mutex::new(None),
            active_model: Mutex::new(None),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os: String,
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub total_ram_gb: f64,
    pub free_ram_gb: f64,
    pub os_reserved_gb: f64,
    pub gpu_name: Option<String>,
    pub gpu_vram_total_gb: f64,
    pub gpu_vram_free_gb: f64,
    pub ollama_installed: bool,
    pub ollama_running: bool,
    pub models_dir: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PullProgress {
    pub model: String,
    pub status: String,
    pub completed: u64,
    pub total: u64,
    pub percent: f64,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct ChatMessageIn {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub conv_id: String,
    pub model: String,
    pub messages: Vec<ChatMessageIn>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub conv_id: String,
    pub delta: String,
    pub done: bool,
    pub error: Option<String>,
}

fn find_ollama_binary() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if Command::new("where").arg("ollama").output().map(|o| o.status.success()).unwrap_or(false) {
            return Some("ollama".to_string());
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("Programs").join("Ollama").join("ollama.exe");
            if p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if Command::new("which").arg("ollama").output().map(|o| o.status.success()).unwrap_or(false) {
            return Some("ollama".to_string());
        }
        for p in ["/usr/local/bin/ollama", "/usr/bin/ollama"] {
            if PathBuf::from(p).is_file() {
                return Some(p.to_string());
            }
        }
    }
    None
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn detect_gpu() -> (Option<String>, f64, f64) {
    let out = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return (None, 0.0, 0.0),
    };
    let s = String::from_utf8_lossy(&out.stdout);
    let Some(line) = s.lines().next() else {
        return (None, 0.0, 0.0);
    };
    let parts: Vec<&str> = line.split(',').map(|p| p.trim()).collect();
    if parts.is_empty() || parts[0].is_empty() {
        return (None, 0.0, 0.0);
    }
    let name = parts[0].to_string();
    let total_mib: f64 = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let free_mib: f64 = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    (
        Some(name),
        round1(total_mib / 1024.0),
        round1(free_mib / 1024.0),
    )
}

fn default_models_dir() -> String {
    if let Ok(v) = std::env::var("OLLAMA_MODELS") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return PathBuf::from(home).join(".ollama").join("models").to_string_lossy().to_string();
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".ollama").join("models").to_string_lossy().to_string();
        }
    }
    "~/.ollama/models".to_string()
}

async fn is_ollama_up() -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1200))
        .build()
        .ok();
    let Some(c) = client else { return false };
    c.get(format!("{OLLAMA_URL}/api/tags"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

#[tauri::command]
async fn get_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_else(|| "CPU".to_string());
    let total_gb = sys.total_memory() as f64 / 1_073_741_824.0;
    let free_gb = sys.available_memory() as f64 / 1_073_741_824.0;
    let base_os_reserve = if cfg!(target_os = "windows") { 2.0 } else { 1.5 };
    let os_reserved = round1((total_gb * 0.12).max(base_os_reserve).min(total_gb * 0.4));
    let (gpu_name, gpu_vram_total_gb, gpu_vram_free_gb) = detect_gpu();

    SystemInfo {
        os: std::env::consts::OS.to_string(),
        cpu_name,
        cpu_cores: sys.cpus().len(),
        total_ram_gb: round1(total_gb),
        free_ram_gb: round1(free_gb),
        os_reserved_gb: os_reserved,
        gpu_name,
        gpu_vram_total_gb,
        gpu_vram_free_gb,
        ollama_installed: find_ollama_binary().is_some(),
        ollama_running: is_ollama_up().await,
        models_dir: default_models_dir(),
    }
}

#[tauri::command]
async fn start_ollama(state: State<'_, AppState>) -> Result<bool, String> {
    if is_ollama_up().await {
        return Ok(true);
    }
    let bin = find_ollama_binary().ok_or("Ollama não encontrado neste computador.")?;
    let mut cmd = Command::new(bin);
    cmd.arg("serve");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let child = cmd.spawn().map_err(|e| format!("Falha ao iniciar Ollama: {e}"))?;
    if let Ok(mut guard) = state.ollama_child.lock() {
        *guard = Some(child);
    }

    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if is_ollama_up().await {
            return Ok(true);
        }
    }
    Err("O motor Ollama demorou para responder na porta 11434.".to_string())
}

#[tauri::command]
async fn list_installed_models() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{OLLAMA_URL}/api/tags"))
        .await
        .map_err(|e| e.to_string())?;
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_running_models() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{OLLAMA_URL}/api/ps"))
        .await
        .map_err(|e| e.to_string())?;
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn unload_model(model: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    client
        .post(format!("{OLLAMA_URL}/api/generate"))
        .json(&serde_json::json!({ "model": model, "keep_alive": 0 }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn delete_model(model: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{OLLAMA_URL}/api/delete"))
        .json(&serde_json::json!({ "name": model }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Erro ao remover modelo (HTTP {})", resp.status()));
    }
    Ok(())
}

#[tauri::command]
async fn pull_model(app: AppHandle, model: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{OLLAMA_URL}/api/pull"))
        .json(&serde_json::json!({ "name": model, "stream": true }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Erro ao baixar (HTTP {})", resp.status()));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                    let _ = app.emit("pull-progress", PullProgress {
                        model: model.clone(),
                        status: "erro".into(),
                        completed: 0,
                        total: 0,
                        percent: 0.0,
                        done: true,
                        error: Some(err.to_string()),
                    });
                    return Err(err.to_string());
                }
                let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string();
                let completed = v.get("completed").and_then(|n| n.as_u64()).unwrap_or(0);
                let total = v.get("total").and_then(|n| n.as_u64()).unwrap_or(0);
                let percent = if total > 0 { (completed as f64 / total as f64) * 100.0 } else { 0.0 };
                let _ = app.emit("pull-progress", PullProgress {
                    model: model.clone(),
                    status: status.clone(),
                    completed,
                    total,
                    percent,
                    done: status == "success",
                    error: None,
                });
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn cancel_chat(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut guard) = state.chat_cancel.lock() {
        if let Some(tx) = guard.take() {
            let _ = tx.send(true);
        }
    }
    Ok(())
}

#[tauri::command]
async fn chat_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    req: ChatRequest,
) -> Result<(), String> {
    let (tx, mut rx) = tokio::sync::watch::channel(false);
    if let Ok(mut guard) = state.chat_cancel.lock() {
        if let Some(prev) = guard.replace(tx) {
            let _ = prev.send(true);
        }
    }
    if let Ok(mut m) = state.active_model.lock() {
        *m = Some(req.model.clone());
    }

    let client = reqwest::Client::new();
    let msgs: Vec<serde_json::Value> = req
        .messages
        .iter()
        .map(|m| {
            if m.images.is_empty() {
                serde_json::json!({ "role": m.role, "content": m.content })
            } else {
                serde_json::json!({ "role": m.role, "content": m.content, "images": m.images })
            }
        })
        .collect();

    let send_fut = client
        .post(format!("{OLLAMA_URL}/api/chat"))
        .json(&serde_json::json!({
            "model": req.model,
            "messages": msgs,
            "stream": true
        }))
        .send();

    let resp = tokio::select! {
        _ = rx.changed() => {
            let _ = app.emit("chat-event", ChatEvent {
                conv_id: req.conv_id,
                delta: String::new(),
                done: true,
                error: None,
            });
            return Ok(());
        }
        r = send_fut => r.map_err(|e| e.to_string())?,
    };

    if !resp.status().is_success() {
        let msg = format!("Erro do motor local (HTTP {})", resp.status());
        let _ = app.emit("chat-event", ChatEvent {
            conv_id: req.conv_id,
            delta: String::new(),
            done: true,
            error: Some(msg.clone()),
        });
        return Err(msg);
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    loop {
        let next_chunk = tokio::select! {
            _ = rx.changed() => {
                drop(stream);
                let _ = app.emit("chat-event", ChatEvent {
                    conv_id: req.conv_id.clone(),
                    delta: String::new(),
                    done: true,
                    error: None,
                });
                break;
            }
            c = stream.next() => c,
        };
        let Some(chunk) = next_chunk else {
            break;
        };
        let bytes = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                let delta = v
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let done = v.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
                let _ = app.emit("chat-event", ChatEvent {
                    conv_id: req.conv_id.clone(),
                    delta,
                    done,
                    error: None,
                });
            }
        }
    }
    if let Ok(mut guard) = state.chat_cancel.lock() {
        *guard = None;
    }
    Ok(())
}

#[tauri::command]
fn load_conversations(app: AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let file = dir.join("conversations.json");
    let backup = dir.join("conversations.bak.json");

    if let Ok(content) = std::fs::read_to_string(&file) {
        if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
            return Ok(content);
        }
    }
    if let Ok(content) = std::fs::read_to_string(&backup) {
        if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
            return Ok(content);
        }
    }
    Ok("[]".to_string())
}

#[tauri::command]
fn save_conversations(app: AppHandle, data: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&data)
        .map_err(|e| format!("Dados de conversa inválidos: {e}"))?;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let file = dir.join("conversations.json");
    let backup = dir.join("conversations.bak.json");
    let tmp = dir.join("conversations.json.tmp");

    if file.is_file() {
        let _ = std::fs::copy(&file, &backup);
    }
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &file).map_err(|e| e.to_string())
}

fn cleanup_on_exit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let had_active_chat = if let Ok(mut guard) = state.chat_cancel.lock() {
        if let Some(tx) = guard.take() {
            let _ = tx.send(true);
            true
        } else {
            false
        }
    } else {
        false
    };

    if let Ok(mut child_guard) = state.ollama_child.lock() {
        if let Some(mut child) = child_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    if had_active_chat {
        if let Ok(m_guard) = state.active_model.lock() {
            if let Some(model) = m_guard.as_ref().cloned() {
                tauri::async_runtime::block_on(async move {
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_millis(700))
                        .build();
                    if let Ok(c) = client {
                        let _ = c
                            .post(format!("{OLLAMA_URL}/api/generate"))
                            .json(&serde_json::json!({ "model": model, "keep_alive": 0 }))
                            .send()
                            .await;
                    }
                });
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                cleanup_on_exit(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            start_ollama,
            list_installed_models,
            list_running_models,
            unload_model,
            delete_model,
            pull_model,
            cancel_chat,
            chat_stream,
            load_conversations,
            save_conversations
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Guanaco");
}
