//! Tauri backend: thin commands over the `astro` lib. The webview only
//! renders; every capability (ephemeris, gazetteer, whisper, routing,
//! emission) runs natively here, exactly as in the CLI/TUI.

use astro::chart::{BirthInput, parse_time};
use astro::contract::ChartData;
use astro::{TranscriptSource, geo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// The last built chart stays backend-side, so saving the artifact never
/// round-trips `ChartData` through the webview (the contract stays
/// Serialize-only).
#[derive(Default)]
struct AppState(Mutex<Option<ChartData>>);

#[derive(Serialize)]
struct PlaceDto {
    id: u32,
    label: String,
    tz: String,
    lat: f64,
    lon: f64,
}

#[tauri::command]
fn search_places(query: String) -> Vec<PlaceDto> {
    geo::search(&query, 6)
        .into_iter()
        .map(|p| PlaceDto {
            id: p.id,
            label: p.label(),
            tz: p.tz.to_string(),
            lat: p.lat,
            lon: p.lon,
        })
        .collect()
}

#[derive(Deserialize)]
struct BirthForm {
    name: String,
    date: String,
    time: String,
    place_id: u32,
    transcript: Option<String>,
    model: Option<String>,
}

/// Classify the transcript input the same way the TUI does: content-sniffed
/// audio needs a model; anything else is a transcript file.
fn transcript_source(
    transcript: Option<&str>,
    model: Option<&str>,
) -> Result<TranscriptSource, String> {
    let Some(path) = transcript.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(TranscriptSource::None);
    };
    if !Path::new(path).exists() {
        return Err(format!("no file at {path}"));
    }
    if astro::transcribe::is_audio(Path::new(path)) {
        let model = model.map(str::trim).filter(|s| !s.is_empty()).ok_or(
            "an audio transcript needs a ggml whisper model — pick one in the model field",
        )?;
        if !Path::new(model).exists() {
            return Err(format!("no model file at {model}"));
        }
        Ok(TranscriptSource::Audio { wav: PathBuf::from(path), model: PathBuf::from(model) })
    } else {
        Ok(TranscriptSource::File(PathBuf::from(path)))
    }
}

#[tauri::command]
async fn build(
    app: AppHandle,
    state: State<'_, AppState>,
    form: BirthForm,
) -> Result<serde_json::Value, String> {
    let place = geo::by_id(form.place_id).ok_or("pick a place from the suggestions")?;
    let input = BirthInput {
        name: if form.name.trim().is_empty() {
            "Anonymous".into()
        } else {
            form.name.trim().into()
        },
        date: form
            .date
            .parse()
            .map_err(|_| "a date as YYYY-MM-DD, e.g. 1990-07-13".to_string())?,
        time: parse_time(&form.time)?,
        lat: place.lat,
        lon: place.lon,
        tz: place.tz,
        place: place.label(),
    };
    let source = transcript_source(form.transcript.as_deref(), form.model.as_deref())?;

    let progress_app = app.clone();
    let (chart, _n_routed) = tauri::async_runtime::spawn_blocking(move || {
        astro::build_reading(&input, source, move |pct| {
            let _ = progress_app.emit("transcribe-progress", pct);
        })
    })
    .await
    .map_err(|e| format!("build task failed: {e}"))??;

    let json = serde_json::to_value(&chart).map_err(|e| e.to_string())?;
    *state.0.lock().unwrap() = Some(chart);
    Ok(json)
}

#[tauri::command]
fn save_artifact(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let guard = state.0.lock().unwrap();
    let chart = guard.as_ref().ok_or("no chart has been built yet")?;
    let html = astro::emit::emit(chart)?;
    std::fs::write(&path, html).map_err(|e| format!("cannot write {path}: {e}"))?;
    Ok(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|_app| {
            // one-time gazetteer parse off the critical path
            tauri::async_runtime::spawn_blocking(geo::warm);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![search_places, build, save_artifact])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
