//! Tauri backend: thin commands over the `astro` lib. The webview only
//! renders; every capability (ephemeris, gazetteer, whisper, routing,
//! emission) runs natively here, exactly as in the CLI/TUI.

use astro::chart::parse_time;
use astro::contract::ChartData;
use astro::{TranscriptSource, geo};
use serde::{Deserialize, Serialize};
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

// async: keeps the gazetteer scan (and a possible cold-parse stall on the
// very first keystroke) off the main thread
#[tauri::command]
async fn search_places(query: String) -> Vec<PlaceDto> {
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

#[tauri::command]
async fn build(
    app: AppHandle,
    state: State<'_, AppState>,
    form: BirthForm,
) -> Result<ChartData, String> {
    let place = geo::by_id(form.place_id).ok_or("pick a place from the suggestions")?;
    let date = form
        .date
        .parse()
        .map_err(|_| "a date as YYYY-MM-DD, e.g. 1990-07-13".to_string())?;
    let input = astro::birth_at_place(&form.name, date, parse_time(&form.time)?, place);
    let source = TranscriptSource::classify(
        form.transcript.as_deref().unwrap_or(""),
        form.model.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;

    let progress_app = app.clone();
    let (chart, _n_routed) = tauri::async_runtime::spawn_blocking(move || {
        astro::build_reading(&input, source, move |pct| {
            let _ = progress_app.emit("transcribe-progress", pct);
        })
    })
    .await
    .map_err(|e| format!("build task failed: {e}"))??;

    *state.0.lock().unwrap() = Some(chart.clone());
    Ok(chart)
}

// async: rendering + disk write stay off the main thread
#[tauri::command]
async fn save_artifact(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let guard = state.0.lock().unwrap();
    let chart = guard.as_ref().ok_or("no chart has been built yet")?;
    astro::emit::write_artifact(chart, path.as_ref())?;
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
