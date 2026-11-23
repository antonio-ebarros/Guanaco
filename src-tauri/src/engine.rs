use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use tauri::AppHandle;

use crate::llamafile::LlamafileEngine;
use crate::ollama::{AppState, OllamaEngine};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub custom_system_prompt: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub num_ctx: Option<u32>,
    #[serde(default)]
    pub repeat_penalty: Option<f64>,
    #[serde(default)]
    pub num_gpu: Option<i32>,
    #[serde(default)]
    pub knowledge_folder: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub conv_id: String,
    pub delta: String,
    pub done: bool,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<crate::tools::RagSource>>,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelRequest {
    pub name: String,
    pub from_source: String,
    pub is_gguf_file: bool,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub num_ctx: Option<u32>,
}

/// Interface comum para motores locais de IA (Ollama, llamafile, llama.cpp, etc.).
pub trait AiEngine: Sync {
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
    fn is_installed(&self, app: &AppHandle) -> bool;
    fn is_running(&self) -> BoxFuture<'_, bool>;
    fn start<'a>(&'a self, app: &'a AppHandle, state: &'a AppState) -> BoxFuture<'a, Result<bool, String>>;
    fn install<'a>(&'a self, app: &'a AppHandle, state: &'a AppState) -> BoxFuture<'a, Result<bool, String>>;
    fn list_installed(&self) -> BoxFuture<'_, Result<serde_json::Value, String>>;
    fn list_running(&self) -> BoxFuture<'_, Result<serde_json::Value, String>>;
    fn unload_model(&self, model: String) -> BoxFuture<'_, Result<(), String>>;
    fn delete_model(&self, model: String) -> BoxFuture<'_, Result<(), String>>;
    fn pull_model<'a>(&'a self, app: &'a AppHandle, model: String) -> BoxFuture<'a, Result<(), String>>;
    fn chat_stream<'a>(
        &'a self,
        app: &'a AppHandle,
        state: &'a AppState,
        req: ChatRequest,
    ) -> BoxFuture<'a, Result<(), String>>;
    fn cancel_chat<'a>(&'a self, state: &'a AppState) -> BoxFuture<'a, Result<(), String>>;
}

static OLLAMA: OllamaEngine = OllamaEngine;
static LLAMAFILE: LlamafileEngine = LlamafileEngine;
static ACTIVE_KIND: AtomicU8 = AtomicU8::new(0); // 0 = ollama, 1 = llamafile
static LLAMAFILE_PATH: Mutex<String> = Mutex::new(String::new());

pub fn set_active_engine_state(engine: &str, llamafile_path: &str) {
    let kind = if engine == "llamafile" { 1 } else { 0 };
    ACTIVE_KIND.store(kind, Ordering::Relaxed);
    if let Ok(mut guard) = LLAMAFILE_PATH.lock() {
        *guard = llamafile_path.to_string();
    }
}

pub fn active_engine_kind() -> &'static str {
    if ACTIVE_KIND.load(Ordering::Relaxed) == 1 {
        "llamafile"
    } else {
        "ollama"
    }
}

pub fn current_llamafile_path() -> String {
    LLAMAFILE_PATH
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

pub fn active_engine() -> &'static dyn AiEngine {
    if ACTIVE_KIND.load(Ordering::Relaxed) == 1 {
        &LLAMAFILE
    } else {
        &OLLAMA
    }
}
