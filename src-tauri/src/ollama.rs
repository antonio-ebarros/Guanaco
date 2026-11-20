use futures_util::StreamExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::engine::{
    AiEngine, BoxFuture, ChatEvent, ChatRequest, CreateModelRequest, EngineInstallProgress,
    PullProgress,
};

pub const OLLAMA_URL: &str = "http://127.0.0.1:11434";
const GUANACO_SYSTEM_PROMPT: &str = include_str!("../prompts/guanaco_system.txt");

pub struct OllamaEngine;

pub struct AppState {
    pub chat_cancel: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    pub ollama_child: Mutex<Option<Child>>,
    pub llamafile_child: Mutex<Option<Child>>,
    pub active_model: Mutex<Option<String>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            chat_cancel: Mutex::new(None),
            ollama_child: Mutex::new(None),
            llamafile_child: Mutex::new(None),
            active_model: Mutex::new(None),
        }
    }
}

impl AiEngine for OllamaEngine {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn is_installed(&self, app: &AppHandle) -> bool {
        find_ollama_binary(app).is_some()
    }

    fn is_running(&self) -> BoxFuture<'_, bool> {
        Box::pin(is_ollama_up())
    }

    fn start<'a>(&'a self, app: &'a AppHandle, state: &'a AppState) -> BoxFuture<'a, Result<bool, String>> {
        Box::pin(start_ollama_impl(app, state))
    }

    fn install<'a>(&'a self, app: &'a AppHandle, state: &'a AppState) -> BoxFuture<'a, Result<bool, String>> {
        Box::pin(install_ollama_impl(app, state))
    }

    fn list_installed(&self) -> BoxFuture<'_, Result<serde_json::Value, String>> {
        Box::pin(list_installed_models_impl())
    }

    fn list_running(&self) -> BoxFuture<'_, Result<serde_json::Value, String>> {
        Box::pin(list_running_models_impl())
    }

    fn unload_model(&self, model: String) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(unload_model_impl(model))
    }

    fn delete_model(&self, model: String) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(delete_model_impl(model))
    }

    fn pull_model<'a>(&'a self, app: &'a AppHandle, model: String) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(pull_model_impl(app, model))
    }

    fn chat_stream<'a>(
        &'a self,
        app: &'a AppHandle,
        state: &'a AppState,
        req: ChatRequest,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(chat_stream_impl(app, state, req))
    }

    fn cancel_chat<'a>(&'a self, state: &'a AppState) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(cancel_chat_impl(state))
    }
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

pub async fn start_ollama_impl(app: &AppHandle, state: &AppState) -> Result<bool, String> {
    if is_ollama_up().await {
        return Ok(true);
    }
    let bin = find_ollama_binary(app).ok_or("Ollama não encontrado neste computador.")?;
    let mut cmd = Command::new(&bin);
    cmd.arg("serve");
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(eng) = engine_dir(app) {
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
pub async fn start_ollama(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    crate::engine::active_engine().start(&app, &state).await
}

pub async fn install_ollama_impl(app: &AppHandle, state: &AppState) -> Result<bool, String> {
    if find_ollama_binary(app).is_some() {
        return start_ollama_impl(app, state).await;
    }

    let eng = engine_dir(app).ok_or("Pasta de dados do usuário indisponível.")?;
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
    let started = start_ollama_impl(app, state).await?;
    emit_prog("Motor pronto!", total, total, true, None);
    Ok(started)
}

#[tauri::command]
pub async fn install_ollama(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    crate::engine::active_engine().install(&app, &state).await
}

pub async fn list_installed_models_impl() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{OLLAMA_URL}/api/tags"))
        .await
        .map_err(|e| e.to_string())?;
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_installed_models() -> Result<serde_json::Value, String> {
    crate::engine::active_engine().list_installed().await
}

pub async fn list_running_models_impl() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{OLLAMA_URL}/api/ps"))
        .await
        .map_err(|e| e.to_string())?;
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_running_models() -> Result<serde_json::Value, String> {
    crate::engine::active_engine().list_running().await
}

pub async fn unload_model_impl(model: String) -> Result<(), String> {
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
pub async fn unload_model(model: String) -> Result<(), String> {
    crate::engine::active_engine().unload_model(model).await
}

pub async fn delete_model_impl(model: String) -> Result<(), String> {
    let _ = unload_model_impl(model.clone()).await;
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
pub async fn delete_model(model: String) -> Result<(), String> {
    crate::engine::active_engine().delete_model(model).await
}

pub async fn pull_model_impl(app: &AppHandle, model: String) -> Result<(), String> {
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
pub async fn pull_model(app: AppHandle, model: String) -> Result<(), String> {
    crate::engine::active_engine().pull_model(&app, model).await
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

pub async fn cancel_chat_impl(state: &AppState) -> Result<(), String> {
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
pub async fn cancel_chat(state: State<'_, AppState>) -> Result<(), String> {
    crate::engine::active_engine().cancel_chat(&state).await
}

pub async fn chat_stream_impl(
    app: &AppHandle,
    state: &AppState,
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

    let pro_active = crate::license::is_pro_active(app);
    let client = reqwest::Client::new();
    let mut msgs: Vec<serde_json::Value> = Vec::with_capacity(req.messages.len() + 1);
    if !req.messages.iter().any(|m| m.role == "system") {
        let profile_rule = match req.profile.as_str() {
            "free" if pro_active => "Perfil Livre (Guanaco Pro): responda de forma direta, madura e sem filtros morais ou restrições de estilo desnecessárias, priorizando a liberdade criativa, técnica e analítica do usuário.",
            "balanced" => "Perfil Equilibrado: permite linguagem informal, humor ácido e discussão madura sobre temas complexos de forma responsável, evitando conteúdo sexual explícito ou incentivo a danos reais.",
            _ => "Perfil Conservador: mantenha linguagem apropriada para todas as idades (incluindo menores de idade). Recuse gerar conteúdo adulto explícito, violência gráfica ou instruções perigosas, e alerte contra riscos físicos no dia a dia.",
        };
        let custom_part = if pro_active {
            req.custom_system_prompt
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n\nInstruções personalizadas do usuário (System Prompt Pro):\n{s}"))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let last_user_query = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let tools_part = if pro_active {
            crate::tools::build_tools_augmentation(
                last_user_query,
                req.enable_smart_tools,
                req.knowledge_folder.as_deref(),
            )
            .map(|t| format!("\n\nDados verificados pelas Ferramentas Locais do Guanaco Pro:\n{t}"))
            .unwrap_or_default()
        } else {
            String::new()
        };
        let sys_content = format!(
            "{}\n{}\nModelo ativo nesta conversa: {}.{}{}",
            GUANACO_SYSTEM_PROMPT.trim(),
            profile_rule,
            req.model,
            custom_part,
            tools_part
        );
        msgs.push(serde_json::json!({ "role": "system", "content": sys_content }));
    }
    for m in &req.messages {
        if m.images.is_empty() {
            msgs.push(serde_json::json!({ "role": m.role, "content": m.content }));
        } else {
            msgs.push(serde_json::json!({ "role": m.role, "content": m.content, "images": m.images }));
        }
    }

    let temp = if pro_active {
        req.temperature.unwrap_or(0.35).clamp(0.0, 2.0)
    } else {
        0.35
    };
    let mut options = serde_json::json!({
        "temperature": temp
    });
    if pro_active {
        if let Some(tp) = req.top_p {
            options["top_p"] = serde_json::json!(tp.clamp(0.05, 1.0));
        }
        if let Some(ctx) = req.num_ctx {
            options["num_ctx"] = serde_json::json!(ctx.clamp(512, 131072));
        }
        if let Some(rp) = req.repeat_penalty {
            options["repeat_penalty"] = serde_json::json!(rp.clamp(0.8, 2.0));
        }
        if let Some(ng) = req.num_gpu {
            if ng >= 0 {
                options["num_gpu"] = serde_json::json!(ng);
            }
        }
    }
    if req.eco_mode {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let eco_threads = (cores / 2).max(2);
        options["num_thread"] = serde_json::json!(eco_threads);
        if !pro_active || req.num_ctx.is_none() {
            options["num_ctx"] = serde_json::json!(2048);
        }
    }
    let payload = serde_json::json!({
        "model": req.model,
        "messages": msgs,
        "stream": true,
        "options": options
    });

    let send_fut = client
        .post(format!("{OLLAMA_URL}/api/chat"))
        .json(&payload)
        .send();

    let mut resp = tokio::select! {
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

    // Auto-fallback inteligente: se o usuário forçou 100% GPU (num_gpu > 0) ou contexto alto
    // e o modelo excedeu a VRAM da placa de vídeo (HTTP 500), reenvia em modo Automático (GPU + RAM).
    if !resp.status().is_success() && pro_active && req.num_gpu.unwrap_or(-1) > 0 {
        let mut fallback_payload = payload.clone();
        if let Some(opts) = fallback_payload.get_mut("options").and_then(|o| o.as_object_mut()) {
            opts.remove("num_gpu");
        }
        let retry_fut = client
            .post(format!("{OLLAMA_URL}/api/chat"))
            .json(&fallback_payload)
            .send();
        if let Ok(r2) = retry_fut.await {
            resp = r2;
        }
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp
            .text()
            .await
            .ok()
            .and_then(|body| {
                serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            })
            .unwrap_or_default();
        let msg = if detail.to_lowercase().contains("memory") || detail.to_lowercase().contains("alloc") || status.as_u16() == 500 {
            format!(
                "Memória VRAM/RAM insuficiente para carregar este modelo com os parâmetros atuais (HTTP {}). Dica: em Controle → Guanaco Pro, deixe 'Camadas na GPU' em Automático ou reduza a Janela de Contexto.",
                status
            )
        } else if !detail.is_empty() {
            format!("Erro do motor local ({status}): {detail}")
        } else {
            format!("Erro do motor local (HTTP {status})")
        };
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
pub async fn chat_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    req: ChatRequest,
) -> Result<(), String> {
    crate::engine::active_engine().chat_stream(&app, &state, req).await
}

#[tauri::command]
pub async fn pick_gguf_file() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| -> Result<Option<String>, String> {
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(out) = Command::new("zenity")
                .args([
                    "--file-selection",
                    "--title=Escolher modelo GGUF (.gguf)",
                    "--file-filter=Modelos GGUF (*.gguf) | *.gguf *.GGUF",
                    "--file-filter=Todos os arquivos | *",
                ])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Ok(Some(s));
                    }
                }
                return Ok(None);
            }
            if let Ok(out) = Command::new("kdialog")
                .args(["--getopenfilename", ".", "*.gguf|Modelos GGUF (*.gguf)"])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Ok(Some(s));
                    }
                }
                return Ok(None);
            }
            Err("Nenhum seletor gráfico (zenity/kdialog) encontrado. Digite ou cole o caminho do arquivo .gguf no campo.".to_string())
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let script = "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Filter = 'Modelos GGUF (*.gguf)|*.gguf|Todos (*.*)|*.*'; if ($f.ShowDialog() -eq 'OK') { [Console]::Out.Write($f.FileName) }";
            let out = Command::new("powershell")
                .args(["-NoProfile", "-Sta", "-Command", script])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| e.to_string())?;
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                Ok(None)
            } else {
                Ok(Some(s))
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn expand_home_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

#[tauri::command]
pub async fn create_custom_model(
    app: AppHandle,
    req: CreateModelRequest,
) -> Result<String, String> {
    if !crate::license::is_pro_active(&app) {
        return Err("Recurso exclusivo do Guanaco Pro. Ative sua licença na aba Controle.".to_string());
    }

    let clean_name = req.name.trim().to_lowercase();
    if clean_name.is_empty()
        || !clean_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        return Err("Nome inválido. Use apenas letras, números, hífen (-), sublinhado (_) ou dois-pontos (:).".to_string());
    }

    let from_target = if req.is_gguf_file {
        let p = expand_home_path(&req.from_source);
        if !p.is_file() {
            return Err(format!("Arquivo .gguf não encontrado em: {}", p.display()));
        }
        let abs = std::fs::canonicalize(&p).unwrap_or(p);
        format!("\"{}\"", abs.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        let base = req.from_source.trim();
        if base.is_empty() {
            return Err("Informe o modelo base ou o arquivo .gguf.".to_string());
        }
        base.to_string()
    };

    let mut modelfile = format!("FROM {from_target}\n");
    if let Some(temp) = req.temperature {
        modelfile.push_str(&format!("PARAMETER temperature {:.2}\n", temp.clamp(0.0, 2.0)));
    }
    if let Some(ctx) = req.num_ctx {
        modelfile.push_str(&format!("PARAMETER num_ctx {}\n", ctx.clamp(512, 131072)));
    }
    if let Some(sys) = req.system_prompt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let safe_sys = sys.replace("\"\"\"", "\"\"");
        modelfile.push_str(&format!("SYSTEM \"\"\"\n{safe_sys}\n\"\"\"\n"));
    }

    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    let mf_path = data_dir.join(format!("Modelfile-{}.tmp", std::process::id()));
    std::fs::write(&mf_path, &modelfile).map_err(|e| e.to_string())?;

    let bin = find_ollama_binary(&app).ok_or("Motor Ollama não encontrado.")?;
    let eng_opt = engine_dir(&app);
    let model_out = clean_name.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut cmd = Command::new(&bin);
        cmd.args(["create", &clean_name, "-f"])
            .arg(&mf_path);
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(eng) = eng_opt {
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
            cmd.creation_flags(0x08000000);
        }
        let out = cmd.output().map_err(|e| format!("Falha ao executar ollama create: {e}"))?;
        let _ = std::fs::remove_file(&mf_path);
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            return Err(format!("Falha ao importar/criar modelo: {detail}"));
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?;

    result?;
    Ok(model_out)
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
    crate::llamafile::stop_llamafile_child(&state);

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
