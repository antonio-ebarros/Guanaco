use serde::{Deserialize, Serialize};
use std::time::Duration;

const RELEASES_LATEST_API: &str =
    "https://api.github.com/repos/antonio-ebarros/Guanaco/releases/latest";
const RELEASES_LATEST_PAGE: &str =
    "https://github.com/antonio-ebarros/Guanaco/releases/latest";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub download_url: String,
    pub release_url: String,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

fn parse_semver(raw: &str) -> Option<(u64, u64, u64)> {
    let cleaned = raw.trim().trim_start_matches('v').trim_start_matches('V');
    let core = cleaned.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some((major, minor, patch))
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

fn pick_best_asset_url(assets: &[GithubAsset], fallback: &str) -> String {
    let preferred_exts: &[&str] = if cfg!(target_os = "windows") {
        &[".exe", ".msi"]
    } else if cfg!(target_os = "macos") {
        &[".dmg", ".app.tar.gz"]
    } else {
        &[".AppImage", ".deb"]
    };

    for ext in preferred_exts {
        if let Some(asset) = assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(&ext.to_lowercase()))
        {
            return asset.browser_download_url.clone();
        }
    }
    fallback.to_string()
}

#[tauri::command]
pub async fn check_app_update() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .get(RELEASES_LATEST_API)
        .header("User-Agent", format!("Guanaco/{current_version}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Falha ao consultar atualizações: {e}"))?;

    if !res.status().is_success() {
        return Err(format!(
            "Não foi possível verificar atualizações (HTTP {}).",
            res.status()
        ));
    }

    let release: GithubRelease = res
        .json()
        .await
        .map_err(|e| format!("Resposta inválida ao verificar atualizações: {e}"))?;

    let latest_version = release
        .tag_name
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();
    let release_url = release
        .html_url
        .unwrap_or_else(|| RELEASES_LATEST_PAGE.to_string());
    let download_url = pick_best_asset_url(&release.assets, &release_url);
    let has_update = is_newer_version(&current_version, &latest_version);

    Ok(UpdateInfo {
        current_version,
        latest_version,
        has_update,
        download_url,
        release_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_versions_correctly() {
        assert!(is_newer_version("0.4.0", "0.4.1"));
        assert!(is_newer_version("0.4.0", "v0.5.0"));
        assert!(is_newer_version("0.9.9", "1.0.0"));
        assert!(!is_newer_version("0.4.0", "0.4.0"));
        assert!(!is_newer_version("0.5.0", "0.4.9"));
    }
}
