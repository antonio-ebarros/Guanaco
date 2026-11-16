use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter, Manager};

const PIPER_LINUX_URL: &str =
    "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz";
const PIPER_WIN_URL: &str =
    "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatus {
    pub installed: bool,
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

fn piper_bin_path(dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dir.join("piper").join("piper.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        dir.join("piper").join("piper")
    }
}

fn voice_info(lang: &str) -> (&'static str, &'static str) {
    match lang {
        "zh" => (
            "zh_CN-huayan-medium",
            "https://huggingface.co/rhasspy/piper-voices/resolve/main/zh/zh_CN/huayan/medium/zh_CN-huayan-medium.onnx",
        ),
        "en" => (
            "en_US-lessac-medium",
            "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx",
        ),
        _ => (
            "pt_BR-edresson-low",
            "https://huggingface.co/rhasspy/piper-voices/resolve/main/pt/pt_BR/edresson/low/pt_BR-edresson-low.onnx",
        ),
    }
}

fn is_voice_ready(dir: &Path, lang: &str) -> bool {
    let (name, _) = voice_info(lang);
    let voices_dir = dir.join("voices");
    voices_dir.join(format!("{name}.onnx")).is_file()
        && voices_dir.join(format!("{name}.onnx.json")).is_file()
}

#[tauri::command]
pub fn get_tts_status(app: AppHandle, lang: Option<String>) -> TtsStatus {
    let dir = tts_dir(&app).unwrap_or_else(|| PathBuf::from("tts"));
    let l = lang.unwrap_or_else(|| "pt".to_string());
    let installed = piper_bin_path(&dir).is_file() && is_voice_ready(&dir, &l);
    TtsStatus {
        installed,
        dir: dir.to_string_lossy().to_string(),
    }
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
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_emit: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            downloaded += bytes.len() as u64;
            if downloaded - last_emit >= 262_144 || (total > 0 && downloaded >= total) {
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
pub async fn install_piper_tts(app: AppHandle, lang: String) -> Result<bool, String> {
    let dir = tts_dir(&app).ok_or("Pasta de dados indisponível.")?;
    let voices_dir = dir.join("voices");
    std::fs::create_dir_all(&voices_dir).map_err(|e| e.to_string())?;

    if !piper_bin_path(&dir).is_file() {
        #[cfg(not(target_os = "windows"))]
        {
            let archive_path = dir.join("piper.tar.gz");
            download_file(&app, PIPER_LINUX_URL, &archive_path, "Baixando Piper TTS…").await?;
            let dir_c = dir.clone();
            let arc_c = archive_path.clone();
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                use std::os::unix::fs::PermissionsExt;
                let f = std::fs::File::open(&arc_c).map_err(|e| e.to_string())?;
                let gz = flate2::read::GzDecoder::new(f);
                let mut tar = tar::Archive::new(gz);
                tar.unpack(&dir_c).map_err(|e| e.to_string())?;
                let bin = piper_bin_path(&dir_c);
                if bin.is_file() {
                    let _ = std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755));
                }
                let _ = std::fs::remove_file(&arc_c);
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())??;
        }
        #[cfg(target_os = "windows")]
        {
            let archive_path = dir.join("piper.zip");
            download_file(&app, PIPER_WIN_URL, &archive_path, "Downloading Piper TTS…").await?;
            let dir_c = dir.clone();
            let arc_c = archive_path.clone();
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                let f = std::fs::File::open(&arc_c).map_err(|e| e.to_string())?;
                let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
                zip.extract(&dir_c).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&arc_c);
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())??;
        }
    }

    let (voice_name, onnx_url) = voice_info(&lang);
    let onnx_dest = voices_dir.join(format!("{voice_name}.onnx"));
    let json_dest = voices_dir.join(format!("{voice_name}.onnx.json"));
    let json_url = format!("{onnx_url}.json");

    if !onnx_dest.is_file() {
        download_file(&app, onnx_url, &onnx_dest, "Baixando voz natural…").await?;
    }
    if !json_dest.is_file() {
        download_file(&app, &json_url, &json_dest, "Baixando config da voz…").await?;
    }

    let _ = app.emit(
        "tts-install-progress",
        TtsInstallProgress {
            status: "Voz pronta!".to_string(),
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
pub async fn synthesize_piper(
    app: AppHandle,
    text: String,
    lang: String,
) -> Result<String, String> {
    let dir = tts_dir(&app).ok_or("Pasta de dados indisponível.")?;
    let bin = piper_bin_path(&dir);
    if !bin.is_file() || !is_voice_ready(&dir, &lang) {
        return Err("Voz natural não instalada para este idioma.".to_string());
    }

    let (voice_name, _) = voice_info(&lang);
    let model_path = dir.join("voices").join(format!("{voice_name}.onnx"));
    let out_wav = dir.join(format!("out-{}.wav", std::process::id()));

    let wav_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let mut cmd = Command::new(&bin);
        cmd.arg("--model")
            .arg(&model_path)
            .arg("--output_file")
            .arg(&out_wav);
        if let Some(parent) = bin.parent() {
            let espeak_dir = parent.join("espeak-ng-data");
            if espeak_dir.is_dir() {
                cmd.arg("--espeak_data").arg(espeak_dir);
            }
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(parent) = bin.parent() {
                let prev = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
                let joined = if prev.is_empty() {
                    parent.to_string_lossy().to_string()
                } else {
                    format!("{}:{prev}", parent.to_string_lossy())
                };
                cmd.env("LD_LIBRARY_PATH", joined);
            }
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Falha ao iniciar Piper: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = std::fs::remove_file(&out_wav);
            return Err(format!("Erro no Piper: {err}"));
        }
        let bytes = std::fs::read(&out_wav).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&out_wav);
        Ok(bytes)
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(format!("data:audio/wav;base64,{}", encode_base64(&wav_bytes)))
}
