//! User preferences: a JSON file in the app-config dir. Every field is
//! optional — an empty preferences file behaves exactly like no preferences
//! at all (no auto-save, no branding, no prefilled model).

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct Preferences {
    /// Folder holding ggml whisper models (`.bin`).
    pub models_dir: Option<String>,
    /// Full path of the model that prefills the form.
    pub default_model: Option<String>,
    /// Library root: built charts auto-save under `{name}_{date}/` here.
    pub readings_dir: Option<String>,
    /// Practitioner name — artifacts read "prepared by …".
    pub astrologer: Option<String>,
    /// Path to the practitioner's logo image, embedded into artifacts.
    pub logo: Option<String>,
    /// PDF page size, "a4" or "letter" (absent = a4).
    pub page_size: Option<String>,
    /// Default reading language code ("en", "ru") that prefills the birth
    /// form's language selector (absent = en).
    pub default_locale: Option<String>,
}

fn path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("no app config dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir.join("preferences.json"))
}

/// Missing or unreadable preferences are simply defaults — the file appears
/// on the first save.
pub fn load(app: &tauri::AppHandle) -> Preferences {
    path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(app: &tauri::AppHandle, prefs: &Preferences) -> Result<(), String> {
    let path = path(app)?;
    let json = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Embed the logo as a `data:` URI so the artifact stays self-contained.
/// Best-effort: a missing or unrecognized file must never fail a build.
pub fn logo_data_uri(path: &Path) -> Option<String> {
    let mime = match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_uri_carries_the_mime_and_survives_missing_files() {
        let dir = std::env::temp_dir();
        let png = dir.join("astro-prefs-test-logo.png");
        std::fs::write(&png, [0x89, b'P', b'N', b'G']).unwrap();
        let uri = logo_data_uri(&png).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"), "{uri}");
        assert!(logo_data_uri(&dir.join("astro-prefs-test-missing.png")).is_none());
        assert!(logo_data_uri(&dir.join("astro-prefs-test-logo.tiff")).is_none());
        std::fs::remove_file(png).ok();
    }
}
