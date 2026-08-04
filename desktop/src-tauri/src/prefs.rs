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
    /// Default house-system code ("whole-sign", "placidus", …) that prefills
    /// the form's house-system selector (absent = whole-sign).
    pub default_house_system: Option<String>,
    /// Default zodiac ("tropical" | "sidereal") that prefills the form (absent
    /// = tropical).
    pub default_zodiac: Option<String>,
    /// Default ayanamsa code ("lahiri", …) that prefills the form's ayanamsa
    /// selector when sidereal (absent = lahiri).
    pub default_ayanamsa: Option<String>,
    /// Gazetteer id of the calculator's last-used place — written when a place
    /// is picked (`set_last_place`), not by the preferences pane.
    pub last_place_id: Option<u32>,
    /// How far this install's saved data has been brought forward. Absent means
    /// "before any migration existed", which is what every existing install
    /// says and what a fresh one says until it saves.
    ///
    /// Not a preference — nothing in the preferences pane sets it, and a person
    /// never sees it. It lives here because this is the one file the app
    /// already owns per install, and a second one for a single integer would be
    /// a second thing to find, back up and lose.
    ///
    /// Absent from the JSON and from the webview's type when unset: the
    /// preferences pane builds a `Preferences` by hand and should not have to
    /// carry a field it neither shows nor may change. `set_preferences` reads
    /// this one from disk for the same reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub schema_version: Option<u32>,
}

/// The migrations that have been written. Bumped when one is added; an install
/// below it runs everything above whatever it says.
pub const SCHEMA_VERSION: u32 = 1;

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

    /// Every existing `preferences.json` predates `schema_version`, and `load`
    /// falls back to `Default` on any parse failure — so a field that refused
    /// to be missing would not fail loudly, it would silently erase someone's
    /// readings folder, their branding and their model path.
    #[test]
    fn a_preferences_file_from_before_a_field_existed_still_reads() {
        let old = r#"{"readings_dir":"/Users/x/Readings","astrologer":"Mira","page_size":"letter"}"#;
        let p: Preferences = serde_json::from_str(old).expect("an older file still parses");
        assert_eq!(p.readings_dir.as_deref(), Some("/Users/x/Readings"));
        assert_eq!(p.astrologer.as_deref(), Some("Mira"));
        assert_eq!(p.schema_version, None, "which is what 'never migrated' means");

        // And an empty file is still exactly no preferences at all.
        assert!(serde_json::from_str::<Preferences>("{}").is_ok());
    }

    /// Unset means absent, not `null` — the webview builds a `Preferences` by
    /// hand and must not have to carry the migration's bookkeeping.
    #[test]
    fn an_unmigrated_install_writes_no_schema_version() {
        let json = serde_json::to_string(&Preferences::default()).unwrap();
        assert!(!json.contains("schema_version"), "{json}");

        let migrated = Preferences { schema_version: Some(SCHEMA_VERSION), ..Default::default() };
        let json = serde_json::to_string(&migrated).unwrap();
        assert!(json.contains(&format!("\"schema_version\":{SCHEMA_VERSION}")), "{json}");
        let back: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema_version, Some(SCHEMA_VERSION));
    }

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
