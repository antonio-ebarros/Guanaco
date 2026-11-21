use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::engine::{
    active_engine_kind, current_llamafile_path, set_active_engine_state, AiEngine, BoxFuture,
    ChatEvent, ChatRequest,
};
use crate::ollama::AppState;

pub const LLAMAFILE_URL: &str = "http://127.0.0.1:8081";
const GUANACO_SYSTEM_PROMPT: &str = include_str!("../prompts/guanaco_system.txt");

pub struct LlamafileEngine;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfig {
    pub engine: String,
    pub llamafile_path: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            engine: "ollama".to_string(),
            llamafile_path: String::new(),
        }
    }
}

fn config_file_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("engine_config.json"))
}

pub fn load_saved_engine_config(app: &AppHandle) -> EngineConfig {
    if !crate::license::is_pro_active(app) {
        set_active_engine_state("ollama", "");
        return EngineConfig::default();
    }
    let Some(path) = config_file_path(app) else {
        return EngineConfig::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return EngineConfig::default();
    };
    let cfg: EngineConfig = serde_json::from_str(&raw).unwrap_or_default();
    set_active_engine_state(&cfg.engine, &cfg.llamafile_path);
    cfg
}

#[tauri::command]
pub fn get_engine_config(app: AppHandle) -> EngineConfig {
    load_saved_engine_config(&app)
}

#[tauri::command]
pub async fn set_engine_config(
    app: AppHandle,
    state: State<'_, AppState>,
    engine: String,
    llamafile_path: String,
) -> Result<EngineConfig, String> {
    if engine == "llamafile" && !crate::license::is_pro_active(&app) {
        return Err("O motor llamafile é exclusivo do Guanaco Pro.".to_string());
    }
    let clean_engine = if engine == "llamafile" {
        "llamafile".to_string()
    } else {
        "ollama".to_string()
    };
    let clean_path = llamafile_path.trim().to_string();
    if clean_engine == "llamafile" && !clean_path.is_empty() {
        let p = expand_home(&clean_path);
        if !p.is_file() {
            return Err(format!("Arquivo llamafile não encontrado em: {}", p.display()));
        }
    }
    let cfg = EngineConfig {
        engine: clean_engine.clone(),
        llamafile_path: clean_path.clone(),
    };
    if let Some(path) = config_file_path(&app) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())?;
    }
    if clean_engine != "llamafile" {
        stop_llamafile_child(&state);
    }
    set_active_engine_state(&clean_engine, &clean_path);
    Ok(cfg)
}

#[tauri::command]
pub async fn pick_llamafile() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| -> Result<Option<String>, String> {
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(out) = Command::new("zenity")
                .args([
                    "--file-selection",
                    "--title=Escolher executável .llamafile ou llama-server",
                    "--file-filter=Llamafile (*.llamafile) | *.llamafile *.exe *",
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
                .args(["--getopenfilename", ".", "*|Llamafile"])
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
            Err("Nenhum seletor gráfico encontrado. Digite o caminho do arquivo .llamafile.".to_string())
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let script = "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Filter = 'Llamafile (*.llamafile;*.exe)|*.llamafile;*.exe|Todos (*.*)|*.*'; if ($f.ShowDialog() -eq 'OK') { [Console]::Out.Write($f.FileName) }";
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

fn expand_home(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

pub fn stop_llamafile_child(state: &AppState) {
    if let Ok(mut guard) = state.llamafile_child.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub async fn is_llamafile_up() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1200))
        .build()
    else {
        return false;
    };
    client
        .get(format!("{LLAMAFILE_URL}/v1/models"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

fn configured_model_info() -> Option<(String, u64, PathBuf)> {
    let raw = current_llamafile_path();
    if raw.trim().is_empty() {
        return None;
    }
    let path = expand_home(&raw);
    if !path.is_file() {
        return None;
    }
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "llamafile".to_string());
    Some((name, size, path))
}

async fn start_llamafile_impl(state: &AppState) -> Result<bool, String> {
    if is_llamafile_up().await {
        return Ok(true);
    }
    let (_, _, bin_path) = configured_model_info().ok_or(
        "Configure o caminho de um arquivo .llamafile válido em Controle → Guanaco Pro.",
    )?;

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755));
    }

    let args = ["--server", "--nobrowser", "--host", "127.0.0.1", "--port", "8081"];
    let mut cmd = Command::new(&bin_path);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        #[cfg(not(target_os = "windows"))]
        Err(_) => {
            // Fallback para binários APE Cosmopolitan via /bin/sh no Linux
            Command::new("/bin/sh")
                .arg(&bin_path)
                .args(args)
                .spawn()
                .map_err(|e| format!("Falha ao iniciar llamafile: {e}"))?
        }
        #[cfg(target_os = "windows")]
        Err(e) => return Err(format!("Falha ao iniciar llamafile: {e}")),
    };

    if let Ok(mut guard) = state.llamafile_child.lock() {
        *guard = Some(child);
    }

    for _ in 0..35 {
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        if is_llamafile_up().await {
            return Ok(true);
        }
    }
    Err("O servidor llamafile demorou para responder em 127.0.0.1:8081.".to_string())
}

async fn list_installed_impl() -> Result<serde_json::Value, String> {
    if let Some((name, size, _)) = configured_model_info() {
        return Ok(serde_json::json!({
            "models": [{ "name": name, "size": size }]
        }));
    }
    if let Ok(resp) = reqwest::get(format!("{LLAMAFILE_URL}/v1/models")).await {
        if let Ok(v) = resp.json::<serde_json::Value>().await {
            if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                let models: Vec<serde_json::Value> = arr
                    .iter()
                    .filter_map(|item| item.get("id").and_then(|id| id.as_str()))
                    .map(|id| serde_json::json!({ "name": id, "size": 0 }))
                    .collect();
                return Ok(serde_json::json!({ "models": models }));
            }
        }
    }
    Ok(serde_json::json!({ "models": [] }))
}

async fn list_running_impl() -> Result<serde_json::Value, String> {
    if !is_llamafile_up().await {
        return Ok(serde_json::json!({ "models": [] }));
    }
    if let Some((name, size, _)) = configured_model_info() {
        return Ok(serde_json::json!({
            "models": [{ "name": name, "size": size }]
        }));
    }
    list_installed_impl().await
}

async fn unload_impl() -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("fuser").args(["-k", "8081/tcp"]).output();
    }
    Ok(())
}

async fn chat_stream_llamafile(
    app: &AppHandle,
    state: &AppState,
    req: ChatRequest,
) -> Result<(), String> {
    if !is_llamafile_up().await {
        let _ = start_llamafile_impl(state).await?;
    }

    let (tx, mut rx) = tokio::sync::watch::channel(false);
    if let Ok(mut guard) = state.chat_cancel.lock() {
        if let Some(prev) = guard.replace(tx) {
            let _ = prev.send(true);
        }
    }

    let pro_active = crate::license::is_pro_active(app);
    let last_user_query = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("");
    let (tools_aug, matched_sources) = crate::tools::build_tools_augmentation(
        last_user_query,
        pro_active,
        req.knowledge_folder.as_deref(),
    );
    if !matched_sources.is_empty() {
        let _ = app.emit(
            "chat-event",
            ChatEvent {
                conv_id: req.conv_id.clone(),
                delta: String::new(),
                done: false,
                error: None,
                sources: Some(matched_sources),
            },
        );
    }

    let mut msgs: Vec<serde_json::Value> = Vec::with_capacity(req.messages.len() + 1);
    let profile_rule = match req.profile.as_str() {
        "free" if pro_active => "Perfil Livre (Guanaco Pro): responda de forma direta, madura e sem filtros morais ou restrições de estilo desnecessárias.",
        "balanced" => "Perfil Equilibrado: permite linguagem informal, humor ácido e discussão madura com responsabilidade.",
        _ => "Perfil Conservador: mantenha linguagem apropriada para todas as idades, evitando conteúdo adulto explícito ou instruções perigosas.",
    };
    let custom_part = if pro_active {
        req.custom_system_prompt
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("\n\nInstruções personalizadas do usuário:\n{s}"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let tools_part = tools_aug
        .map(|t| format!("\n\nDados verificados pelas Ferramentas Locais do Guanaco Pro:\n{t}"))
        .unwrap_or_default();

    let sys_content = format!(
        "{}\n{}\nModelo ativo (motor llamafile): {}.{}{}",
        GUANACO_SYSTEM_PROMPT.trim(),
        profile_rule,
        req.model,
        custom_part,
        tools_part
    );
    msgs.push(serde_json::json!({ "role": "system", "content": sys_content }));
    for m in &req.messages {
        msgs.push(serde_json::json!({ "role": m.role, "content": m.content }));
    }

    let temp = if pro_active {
        req.temperature.unwrap_or(0.35).clamp(0.0, 2.0)
    } else {
        0.35
    };
    let top_p = if pro_active {
        req.top_p.unwrap_or(0.9).clamp(0.05, 1.0)
    } else {
        0.9
    };

    let payload = serde_json::json!({
        "model": req.model,
        "messages": msgs,
        "stream": true,
        "temperature": temp,
        "top_p": top_p
    });

    let client = reqwest::Client::new();
    let send_fut = client
        .post(format!("{LLAMAFILE_URL}/v1/chat/completions"))
        .json(&payload)
        .send();

    let resp = tokio::select! {
        _ = rx.changed() => {
            let _ = app.emit("chat-event", ChatEvent {
                conv_id: req.conv_id,
                delta: String::new(),
                done: true,
                error: None,
                sources: None,
            });
            return Ok(());
        }
        r = send_fut => r.map_err(|e| e.to_string())?,
    };

    if !resp.status().is_success() {
        let msg = format!("Erro do servidor llamafile (HTTP {})", resp.status());
        let _ = app.emit(
            "chat-event",
            ChatEvent {
                conv_id: req.conv_id,
                delta: String::new(),
                done: true,
                error: Some(msg.clone()),
                sources: None,
            },
        );
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
                    sources: None,
                });
                break;
            }
            c = stream.next() => c,
        };
        let Some(chunk) = next_chunk else {
            let _ = app.emit(
                "chat-event",
                ChatEvent {
                    conv_id: req.conv_id.clone(),
                    delta: String::new(),
                    done: true,
                    error: None,
                    sources: None,
                },
            );
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
            let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                continue;
            };
            if data == "[DONE]" {
                let _ = app.emit(
                    "chat-event",
                    ChatEvent {
                        conv_id: req.conv_id.clone(),
                        delta: String::new(),
                        done: true,
                        error: None,
                        sources: None,
                    },
                );
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                let choice = v.get("choices").and_then(|c| c.get(0));
                let delta = choice
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("content"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let finish = choice
                    .and_then(|c| c.get("finish_reason"))
                    .and_then(|f| f.as_str())
                    .is_some();
                if !delta.is_empty() || finish {
                    let _ = app.emit(
                        "chat-event",
                        ChatEvent {
                            conv_id: req.conv_id.clone(),
                            delta,
                            done: finish,
                            error: None,
                            sources: None,
                        },
                    );
                }
            }
        }
    }

    if let Ok(mut guard) = state.chat_cancel.lock() {
        *guard = None;
    }
    Ok(())
}

impl AiEngine for LlamafileEngine {
    fn name(&self) -> &'static str {
        "llamafile"
    }

    fn is_installed(&self, _app: &AppHandle) -> bool {
        let _ = active_engine_kind();
        configured_model_info().is_some()
    }

    fn is_running(&self) -> BoxFuture<'_, bool> {
        Box::pin(is_llamafile_up())
    }

    fn start<'a>(
        &'a self,
        _app: &'a AppHandle,
        state: &'a AppState,
    ) -> BoxFuture<'a, Result<bool, String>> {
        Box::pin(start_llamafile_impl(state))
    }

    fn install<'a>(
        &'a self,
        _app: &'a AppHandle,
        state: &'a AppState,
    ) -> BoxFuture<'a, Result<bool, String>> {
        Box::pin(start_llamafile_impl(state))
    }

    fn list_installed(&self) -> BoxFuture<'_, Result<serde_json::Value, String>> {
        Box::pin(list_installed_impl())
    }

    fn list_running(&self) -> BoxFuture<'_, Result<serde_json::Value, String>> {
        Box::pin(list_running_impl())
    }

    fn unload_model(&self, _model: String) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(unload_impl())
    }

    fn delete_model(&self, _model: String) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async {
            Err("Arquivos .llamafile são executáveis independentes — remova ou troque o arquivo em Controle → Guanaco Pro.".to_string())
        })
    }

    fn pull_model<'a>(
        &'a self,
        _app: &'a AppHandle,
        _model: String,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async {
            Err("No modo llamafile, escolha um arquivo .llamafile local em Controle → Guanaco Pro ou volte para o motor Ollama para baixar modelos.".to_string())
        })
    }

    fn chat_stream<'a>(
        &'a self,
        app: &'a AppHandle,
        state: &'a AppState,
        req: ChatRequest,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(chat_stream_llamafile(app, state, req))
    }

    fn cancel_chat<'a>(&'a self, state: &'a AppState) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let Ok(mut guard) = state.chat_cancel.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(true);
                }
            }
            Ok(())
        })
    }
}
