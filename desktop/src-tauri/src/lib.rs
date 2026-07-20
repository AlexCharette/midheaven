//! Tauri backend: thin commands over the `astro` lib. The webview only
//! renders; every capability (ephemeris, gazetteer, whisper, routing,
//! emission) runs natively here, exactly as in the CLI/TUI.

mod record;

use astro::chart::parse_time;
use astro::contract::{ChartData, Excerpt, Segment};
use astro::route::{LexiconRouter, Router, Transcript, verify_gate};
use astro::{TranscriptSource, geo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

/// Backend-held session: the built chart (so saving never round-trips the
/// webview), the passages it was built with, and the live-recording session
/// accumulated on top of them.
#[derive(Default)]
struct Inner {
    chart: Option<ChartData>,
    /// Excerpts from the original build — session passages append after these.
    base_excerpts: Vec<Excerpt>,
    /// All transcribed segments of this session, time-shifted end to end.
    session: Vec<Segment>,
    session_secs: f64,
    model: Option<PathBuf>,
    recorder: Option<record::Recorder>,
}

#[derive(Default)]
struct AppState(Mutex<Inner>);

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

    let mut inner = state.0.lock().unwrap();
    inner.base_excerpts = chart.excerpts.clone();
    inner.session.clear();
    inner.session_secs = 0.0;
    inner.chart = Some(chart.clone());
    drop(inner);
    Ok(chart)
}

/// Begin capturing the session from the default microphone. The model path
/// comes from the form (the frontend only shows the button when it is set).
#[tauri::command]
fn start_recording(state: State<'_, AppState>, model: String) -> Result<(), String> {
    let model = PathBuf::from(model.trim());
    if !model.exists() {
        return Err(format!("no model file at {}", model.display()));
    }
    let mut inner = state.0.lock().unwrap();
    if inner.recorder.is_some() {
        return Err("already recording".to_string());
    }
    if inner.chart.is_none() {
        return Err("no chart has been built yet".to_string());
    }
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let out = std::env::temp_dir().join(format!("astro-take-{millis}.wav"));
    inner.recorder = Some(record::start(out)?);
    inner.model = Some(model);
    Ok(())
}

/// Stop capturing, transcribe the take (progress on the shared event), and
/// route the whole session's passages into the chart after the build's own.
#[tauri::command]
async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ChartData, String> {
    let (recorder, model, offset) = {
        let mut inner = state.0.lock().unwrap();
        (
            inner.recorder.take().ok_or("not recording")?,
            inner.model.clone().ok_or("no model on record")?,
            inner.session_secs,
        )
    };
    let (wav, secs) = recorder.stop()?;

    let progress_app = app.clone();
    let mut segments = tauri::async_runtime::spawn_blocking(move || {
        astro::transcribe::transcribe(&wav, &model, move |pct| {
            let _ = progress_app.emit("transcribe-progress", pct);
        })
    })
    .await
    .map_err(|e| format!("transcription task failed: {e}"))??;
    for seg in &mut segments {
        seg.start += offset;
    }

    let mut guard = state.0.lock().unwrap();
    let inner = &mut *guard;
    inner.session.extend(segments);
    inner.session_secs = offset + secs;
    let chart = inner.chart.as_mut().ok_or("no chart has been built yet")?;

    let transcript = Transcript::from_segments(inner.session.iter().cloned());
    let vocab = chart.vocab();
    let router = LexiconRouter::new(&vocab, &chart.aspects);
    let mut session_excerpts = verify_gate(&transcript, router.route(&transcript), &vocab);
    for (i, ex) in session_excerpts.iter_mut().enumerate() {
        ex.id = format!("x{}", inner.base_excerpts.len() + i + 1);
    }
    chart.excerpts =
        inner.base_excerpts.iter().cloned().chain(session_excerpts).collect();
    Ok(chart.clone())
}

// async: rendering + disk write stay off the main thread
#[tauri::command]
async fn save_artifact(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let guard = state.0.lock().unwrap();
    let chart = guard.chart.as_ref().ok_or("no chart has been built yet")?;
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
        .invoke_handler(tauri::generate_handler![
            search_places,
            build,
            save_artifact,
            start_recording,
            stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
