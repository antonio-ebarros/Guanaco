use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use tauri::AppHandle;

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

pub fn active_engine() -> &'static dyn AiEngine {
    &OLLAMA
}
