use serde::Serialize;
use std::process::Command;
use sysinfo::System;
use tauri::AppHandle;

use crate::engine::active_engine;
use crate::ollama::default_models_dir;

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

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn detect_gpu() -> (Option<String>, f64, f64) {
    if let Ok(o) = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout);
            if let Some(line) = s.lines().next() {
                let parts: Vec<&str> = line.split(',').map(|p| p.trim()).collect();
                if !parts.is_empty() && !parts[0].is_empty() {
                    let name = parts[0].to_string();
                    let total_mib: f64 = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    let free_mib: f64 = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    return (
                        Some(name),
                        round1(total_mib / 1024.0),
                        round1(free_mib / 1024.0),
                    );
                }
            }
        }
    }

    // Fallback Linux: AMD/Intel DRM sysfs + lspci
    #[cfg(target_os = "linux")]
    {
        let mut vram_total = 0.0;
        let mut vram_free = 0.0;
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("card") || name.contains('-') {
                    continue;
                }
                let dev_dir = entry.path().join("device");
                let total_bytes: u64 = std::fs::read_to_string(dev_dir.join("mem_info_vram_total"))
                    .ok()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
                if total_bytes > 0 {
                    let used_bytes: u64 = std::fs::read_to_string(dev_dir.join("mem_info_vram_used"))
                        .ok()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0);
                    vram_total = round1(total_bytes as f64 / 1_073_741_824.0);
                    vram_free = round1(total_bytes.saturating_sub(used_bytes) as f64 / 1_073_741_824.0);
                    break;
                }
            }
        }
        if let Ok(o) = Command::new("lspci").output() {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout);
                for line in s.lines() {
                    if line.contains("VGA compatible controller:")
                        || line.contains("3D controller:")
                        || line.contains("Display controller:")
                    {
                        if let Some((_, desc)) = line.split_once(": ") {
                            let clean = desc.split(" (rev ").next().unwrap_or(desc).trim().to_string();
                            if !clean.is_empty() {
                                return (Some(clean), vram_total, vram_free);
                            }
                        }
                    }
                }
            }
        }
        if vram_total > 0.0 {
            return (Some("AMD Radeon GPU".to_string()), vram_total, vram_free);
        }
    }

    (None, 0.0, 0.0)
}

#[tauri::command]
pub async fn get_system_info(app: AppHandle) -> SystemInfo {
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

    let engine = active_engine();

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
        ollama_installed: engine.is_installed(&app),
        ollama_running: engine.is_running().await,
        models_dir: default_models_dir(),
    }
}
