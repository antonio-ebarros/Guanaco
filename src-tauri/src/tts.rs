use futures_util::StreamExt;
use serde::Serialize;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tts_rs::engines::kokoro::{KokoroEngine, KokoroInferenceParams, KokoroModelParams};
use tts_rs::SynthesisEngine;

const KOKORO_MODEL_URL: &str =
    "https://github.com/taylorchu/kokoro-onnx/releases/download/v0.2.0/kokoro-quant-convinteger.onnx";
const KOKORO_VOICES_URL: &str =
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin";

pub struct TtsState {
    pub engine: Mutex<Option<KokoroEngine>>,
}

impl Default for TtsState {
    fn default() -> Self {
        Self {
            engine: Mutex::new(None),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatus {
    pub installed: bool,
    pub espeak_ready: bool,
    pub loaded: bool,
    pub dir: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TtsInstallProgress {
    pub status: String,
    pub completed: u64,
    pub total: u64,
    pub percent: f64,
    pub done: bool,
    pub error: Option<String>,
}

fn tts_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("tts"))
}

fn find_espeak_bin() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if Command::new("where")
            .arg("espeak-ng")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("espeak-ng"));
        }
        for p in [
            r"C:\Program Files\eSpeak NG\espeak-ng.exe",
            r"C:\Program Files (x86)\eSpeak NG\espeak-ng.exe",
        ] {
            if PathBuf::from(p).is_file() {
                return Some(PathBuf::from(p));
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        if Command::new("which")
            .arg("espeak-ng")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("espeak-ng"));
        }
        for p in ["/usr/bin/espeak-ng", "/usr/local/bin/espeak-ng"] {
            if PathBuf::from(p).is_file() {
                return Some(PathBuf::from(p));
            }
        }
        None
    }
}

fn models_ready(dir: &PathBuf) -> bool {
    dir.join("kokoro-quant-convinteger.onnx").is_file() && dir.join("voices-v1.0.bin").is_file()
}

#[tauri::command]
pub fn get_tts_status(app: AppHandle, state: State<'_, TtsState>) -> TtsStatus {
    let dir = tts_dir(&app).unwrap_or_else(|| PathBuf::from("tts"));
    let installed = models_ready(&dir);
    let loaded = state.engine.lock().map(|g| g.is_some()).unwrap_or(false);
    TtsStatus {
        installed,
        espeak_ready: find_espeak_bin().is_some(),
        loaded,
        dir: dir.to_string_lossy().to_string(),
    }
}

#[tauri::command]
pub fn unload_kokoro_tts(state: State<'_, TtsState>) -> Result<(), String> {
    if let Ok(mut g) = state.engine.lock() {
        *g = None;
    }
    Ok(())
}

async fn download_file(
    app: &AppHandle,
    url: &str,
    dest: &PathBuf,
    label: &str,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} ao baixar {}", resp.status(), label));
    }
    let total = resp.content_length().unwrap_or(0);
    let tmp = dest.with_extension("tmp");
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_emit: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            downloaded += bytes.len() as u64;
            if downloaded - last_emit >= 524_288 || (total > 0 && downloaded >= total) {
                last_emit = downloaded;
                let percent = if total > 0 {
                    (downloaded as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                let _ = app.emit(
                    "tts-install-progress",
                    TtsInstallProgress {
                        status: label.to_string(),
                        completed: downloaded,
                        total,
                        percent,
                        done: false,
                        error: None,
                    },
                );
            }
        }
    }
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_kokoro_tts(app: AppHandle) -> Result<bool, String> {
    let dir = tts_dir(&app).ok_or("Pasta de dados indisponível.")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let model_path = dir.join("kokoro-quant-convinteger.onnx");
    let voices_path = dir.join("voices-v1.0.bin");

    if !model_path.is_file() {
        download_file(&app, KOKORO_MODEL_URL, &model_path, "Baixando modelo Kokoro…").await?;
    }
    if !voices_path.is_file() {
        download_file(&app, KOKORO_VOICES_URL, &voices_path, "Baixando vozes Kokoro…").await?;
    }

    let _ = app.emit(
        "tts-install-progress",
        TtsInstallProgress {
            status: "Voz Kokoro pronta!".to_string(),
            completed: 1,
            total: 1,
            percent: 100.0,
            done: true,
            error: None,
        },
    );
    Ok(true)
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[tauri::command]
pub async fn synthesize_kokoro(
    app: AppHandle,
    state: State<'_, TtsState>,
    text: String,
    lang: String,
) -> Result<String, String> {
    let dir = tts_dir(&app).ok_or("Pasta de dados indisponível.")?;
    if !models_ready(&dir) {
        return Err("Kokoro não instalado.".to_string());
    }
    let espeak_bin = find_espeak_bin()
        .ok_or("espeak-ng não encontrado no sistema (instale espeak-ng para usar Kokoro).")?;

    let voice = match lang.as_str() {
        "zh" => "zf_xiaobei",
        "en" => "af_heart",
        _ => "pf_dora",
    }
    .to_string();

    let state_arc = state.inner();
    let wav_bytes = tokio::task::block_in_place(|| -> Result<Vec<u8>, String> {
        let mut guard = state_arc
            .engine
            .lock()
            .map_err(|_| "Falha ao acessar motor de voz.".to_string())?;

        if guard.is_none() {
            let mut engine = KokoroEngine::with_espeak(Some(espeak_bin), None);
            let params = KokoroModelParams {
                num_threads: Some(2),
                optimized_model_cache_path: Some(dir.join("kokoro_opt.ort")),
            };
            engine
                .load_model_with_params(&dir, params)
                .map_err(|e| format!("Erro ao carregar Kokoro: {e}"))?;
            *guard = Some(engine);
        }

        let engine = guard.as_mut().unwrap();
        let res = engine
            .synthesize(
                &text,
                Some(KokoroInferenceParams {
                    voice,
                    speed: 1.0,
                    style_index: None,
                }),
            )
            .map_err(|e| format!("Erro ao sintetizar voz: {e}"))?;

        let mut cursor = Cursor::new(Vec::new());
        {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: res.sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
            for s in res.samples {
                let amp = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                writer.write_sample(amp).map_err(|e| e.to_string())?;
            }
            writer.finalize().map_err(|e| e.to_string())?;
        }
        Ok(cursor.into_inner())
    })?;

    Ok(format!("data:audio/wav;base64,{}", encode_base64(&wav_bytes)))
}
