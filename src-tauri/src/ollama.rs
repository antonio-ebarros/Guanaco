use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub const OLLAMA_URL: &str = "http://127.0.0.1:11434";

pub struct AppState {
    pub chat_cancel: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    pub ollama_child: Mutex<Option<Child>>,
    pub active_model: Mutex<Option<String>>,
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
    #[serde(default)]
    pub eco_mode: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub conv_id: String,
    pub delta: String,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EngineInstallProgress {
    pub status: String,
    pub completed: u64,
    pub total: u64,
    pub percent: f64,
    pub done: bool,
    pub error: Option<String>,
}

pub fn engine_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("engine"))
}

pub fn find_ollama_binary(app: &AppHandle) -> Option<String> {
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
        if let Some(eng) = engine_dir(app) {
            for rel in ["bin/ollama", "ollama"] {
                let p = eng.join(rel);
                if p.is_file() {
                    return Some(p.to_string_lossy().to_string());
                }
            }
        }
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

pub fn default_models_dir() -> String {
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

pub async fn is_ollama_up() -> bool {
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
pub async fn start_ollama(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    if is_ollama_up().await {
        return Ok(true);
    }
    let bin = find_ollama_binary(&app).ok_or("Ollama não encontrado neste computador.")?;
    let mut cmd = Command::new(&bin);
    cmd.arg("serve");
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(eng) = engine_dir(&app) {
            let lib_dir = eng.join("lib").join("ollama");
            if lib_dir.is_dir() {
                let prev = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
                let joined = if prev.is_empty() {
                    lib_dir.to_string_lossy().to_string()
                } else {
                    format!("{}:{prev}", lib_dir.to_string_lossy())
                };
                cmd.env("LD_LIBRARY_PATH", joined);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let child = cmd.spawn().map_err(|e| format!("Falha ao iniciar Ollama: {e}"))?;
    if let Ok(mut guard) = state.ollama_child.lock() {
        *guard = Some(child);
    }

    for _ in 0..25 {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if is_ollama_up().await {
            return Ok(true);
        }
    }
    Err("O motor Ollama demorou para responder na porta 11434.".to_string())
}

#[tauri::command]
pub async fn install_ollama(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    if find_ollama_binary(&app).is_some() {
        return start_ollama(app, state).await;
    }

    let eng = engine_dir(&app).ok_or("Pasta de dados do usuário indisponível.")?;
    std::fs::create_dir_all(&eng).map_err(|e| e.to_string())?;

    let emit_prog = |status: &str, completed: u64, total: u64, done: bool, error: Option<String>| {
        let percent = if total > 0 {
            (completed as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let _ = app.emit(
            "engine-install-progress",
            EngineInstallProgress {
                status: status.to_string(),
                completed,
                total,
                percent,
                done,
                error,
            },
        );
    };

    #[cfg(not(target_os = "windows"))]
    let url = "https://github.com/ollama/ollama/releases/latest/download/ollama-linux-amd64.tar.zst";
    #[cfg(target_os = "windows")]
    let url = "https://github.com/ollama/ollama/releases/latest/download/OllamaSetup.exe";

    emit_prog("Conectando ao servidor oficial…", 0, 0, false, None);

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| {
        let msg = format!("Falha ao baixar motor: {e}");
        emit_prog(&msg, 0, 0, true, Some(msg.clone()));
        msg
    })?;

    if !resp.status().is_success() {
        let msg = format!("Erro ao baixar motor (HTTP {})", resp.status());
        emit_prog(&msg, 0, 0, true, Some(msg.clone()));
        return Err(msg);
    }

    let total = resp.content_length().unwrap_or(0);
    #[cfg(not(target_os = "windows"))]
    let tmp_path = eng.join("ollama-download.tar.zst.tmp");
    #[cfg(target_os = "windows")]
    let tmp_path = eng.join("OllamaSetup.exe");

    {
        use std::io::Write;
        let mut file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_emit: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            downloaded += bytes.len() as u64;
            if downloaded - last_emit >= 1_048_576 || (total > 0 && downloaded >= total) {
                last_emit = downloaded;
                emit_prog("Baixando motor de IA local…", downloaded, total, false, None);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        emit_prog("Extraindo arquivos do motor…", total, total, false, None);
        let tmp_clone = tmp_path.clone();
        let eng_clone = eng.clone();
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            use std::os::unix::fs::PermissionsExt;
            let f = std::fs::File::open(&tmp_clone).map_err(|e| e.to_string())?;
            let dec = zstd::stream::read::Decoder::new(f).map_err(|e| e.to_string())?;
            let mut archive = tar::Archive::new(dec);
            archive.unpack(&eng_clone).map_err(|e| e.to_string())?;
            for rel in ["bin/ollama", "ollama"] {
                let bin = eng_clone.join(rel);
                if bin.is_file() {
                    let _ = std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755));
                }
            }
            let _ = std::fs::remove_file(&tmp_clone);
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())??;
    }

    #[cfg(target_os = "windows")]
    {
        emit_prog("Instalando motor no Windows…", total, total, false, None);
        let setup_path = tmp_path.clone();
        let status = tokio::task::spawn_blocking(move || {
            Command::new(&setup_path)
                .args(["/SP-", "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
                .status()
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Falha ao executar instalador: {e}"))?;
        let _ = std::fs::remove_file(&tmp_path);
        if !status.success() {
            return Err("O instalador do Ollama foi cancelado ou falhou.".to_string());
        }
    }

    emit_prog("Ligando motor local…", total, total, false, None);
    let started = start_ollama(app.clone(), state).await?;
    emit_prog("Motor pronto!", total, total, true, None);
    Ok(started)
}

#[tauri::command]
pub async fn list_installed_models() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{OLLAMA_URL}/api/tags"))
        .await
        .map_err(|e| e.to_string())?;
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_running_models() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{OLLAMA_URL}/api/ps"))
        .await
        .map_err(|e| e.to_string())?;
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unload_model(model: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2000))
        .build()
        .map_err(|e| e.to_string())?;
    let _ = client
        .post(format!("{OLLAMA_URL}/api/generate"))
        .json(&serde_json::json!({ "model": model, "keep_alive": 0 }))
        .send()
        .await;
    Ok(())
}

#[tauri::command]
pub async fn delete_model(model: String) -> Result<(), String> {
    let _ = unload_model(model.clone()).await;
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
pub async fn pull_model(app: AppHandle, model: String) -> Result<(), String> {
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

fn stop_stuck_runner() {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("pkill").args(["-TERM", "-x", "llama-server"]).output();
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "ollama_llama_server.exe"])
            .creation_flags(0x08000000)
            .output();
    }
}

#[tauri::command]
pub async fn cancel_chat(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut guard) = state.chat_cancel.lock() {
        if let Some(tx) = guard.take() {
            let _ = tx.send(true);
        }
    }
    let model_opt = state.active_model.lock().ok().and_then(|g| g.clone());
    if let Some(model) = model_opt {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(800))
            .build();
        let ok = if let Ok(c) = client {
            c.post(format!("{OLLAMA_URL}/api/generate"))
                .json(&serde_json::json!({ "model": model, "keep_alive": 0 }))
                .send()
                .await
                .is_ok()
        } else {
            false
        };
        if !ok {
            stop_stuck_runner();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn chat_stream(
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

    let mut payload = serde_json::json!({
        "model": req.model,
        "messages": msgs,
        "stream": true
    });
    if req.eco_mode {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let eco_threads = (cores / 2).max(2);
        payload["options"] = serde_json::json!({
            "num_thread": eco_threads,
            "num_ctx": 2048
        });
    }

    let send_fut = client
        .post(format!("{OLLAMA_URL}/api/chat"))
        .json(&payload)
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

pub fn cleanup_on_exit(app: &AppHandle) {
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

    let used_model = state.active_model.lock().ok().and_then(|mut g| g.take());
    if had_active_chat || used_model.is_some() {
        if let Some(model) = used_model {
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
        stop_stuck_runner();
    }
}
