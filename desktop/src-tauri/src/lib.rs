//! Tauri backend: thin commands over the `astro` lib. The webview only
//! renders; every capability (ephemeris, gazetteer, whisper, routing,
//! emission) runs natively here, exactly as in the CLI/TUI.

mod record;

use astro::chart::parse_time;
use astro::contract::{ChartData, Excerpt, Segment};
use astro::route::{LexiconRouter, Router, Transcript, coalesce, verify_gate};
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
    /// The chart's excerpt list is authoritative — takes append to it and
    /// curation (merge/correct) edits it in place.
    chart: Option<ChartData>,
    /// All transcribed segments of this session, time-shifted end to end
    /// (kept for offset math only).
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
    inner.session.extend(segments.iter().cloned());
    inner.session_secs = offset + secs;
    let chart = inner.chart.as_mut().ok_or("no chart has been built yet")?;

    // Route ONLY the new take and append, so earlier curation (merges,
    // corrections) survives every stop.
    let take = Transcript::from_segments(segments);
    let vocab = chart.vocab();
    let router = LexiconRouter::new(&vocab, &chart.aspects);
    let routed = coalesce(verify_gate(&take, router.route(&take), &vocab), &take);
    let offset_ids = chart.excerpts.len();
    chart.excerpts.extend(routed.into_iter().enumerate().map(|(i, mut ex)| {
        ex.id = format!("x{}", offset_ids + i + 1);
        ex
    }));
    Ok(chart.clone())
}

/// Merge the excerpt into its predecessor: verbatim parts joined, tags
/// unioned, the earlier passage's time anchor kept.
fn merge_up_in(excerpts: &mut Vec<Excerpt>, id: &str) -> Result<(), String> {
    let i = excerpts.iter().position(|e| e.id == id).ok_or("no such passage")?;
    if i == 0 {
        return Err("the first passage has nothing above it to merge into".to_string());
    }
    let cur = excerpts.remove(i);
    let prev = &mut excerpts[i - 1];
    prev.text = format!("{} {}", prev.text, cur.text);
    prev.span[1] = cur.span[1];
    let mut tags: Vec<String> = prev.tags.drain(..).chain(cur.tags).collect();
    tags.sort();
    tags.dedup();
    prev.tags = tags;
    Ok(())
}

/// Amend a passage's text and re-tag it from the corrected words; if the
/// router finds nothing, the previous tags stay (a correction should never
/// make a passage vanish from every section).
fn correct_in(chart: &mut ChartData, id: &str, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("a passage cannot be amended to nothing".to_string());
    }
    let vocab = chart.vocab();
    let router = LexiconRouter::new(&vocab, &chart.aspects);
    let corrected = Transcript::load(text);
    let mut tags: Vec<String> = router
        .route(&corrected)
        .into_iter()
        .flat_map(|raw| raw.tags)
        .collect();
    tags.sort();
    tags.dedup();
    let ex = chart
        .excerpts
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or("no such passage")?;
    ex.text = text.to_string();
    if !tags.is_empty() {
        ex.tags = tags;
    }
    Ok(())
}

#[tauri::command]
fn merge_up(state: State<'_, AppState>, id: String) -> Result<ChartData, String> {
    let mut guard = state.0.lock().unwrap();
    let chart = guard.chart.as_mut().ok_or("no chart has been built yet")?;
    merge_up_in(&mut chart.excerpts, &id)?;
    Ok(chart.clone())
}

#[tauri::command]
fn correct_excerpt(
    state: State<'_, AppState>,
    id: String,
    text: String,
) -> Result<ChartData, String> {
    let mut guard = state.0.lock().unwrap();
    let chart = guard.chart.as_mut().ok_or("no chart has been built yet")?;
    correct_in(chart, &id, &text)?;
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
            stop_recording,
            merge_up,
            correct_excerpt
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart_fixture() -> ChartData {
        let input = astro::chart::BirthInput {
            name: "T".into(),
            date: "1990-07-13".parse().unwrap(),
            time: "14:30:00".parse().unwrap(),
            lat: 52.52,
            lon: 13.405,
            tz: chrono_tz::Europe::Berlin,
            place: "Berlin".into(),
        };
        astro::chart::compute_chart(&input).unwrap()
    }

    fn ex(id: &str, text: &str, tags: &[&str]) -> Excerpt {
        Excerpt {
            id: id.into(),
            time: String::new(),
            span: [0, text.len()],
            text: text.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn merge_up_joins_text_and_unions_tags() {
        let mut list = vec![
            ex("x1", "The sun shines.", &["planet:sun"]),
            ex("x2", "In cancer.", &["sign:cancer", "planet:sun"]),
        ];
        merge_up_in(&mut list, "x2").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].text, "The sun shines. In cancer.");
        assert_eq!(list[0].tags, vec!["planet:sun", "sign:cancer"]);
    }

    #[test]
    fn merge_up_refuses_the_first_passage() {
        let mut list = vec![ex("x1", "Alone.", &["planet:sun"])];
        assert!(merge_up_in(&mut list, "x1").is_err());
        assert!(merge_up_in(&mut list, "x9").is_err());
    }

    #[test]
    fn correction_retags_from_the_corrected_words() {
        let mut chart = chart_fixture();
        chart.excerpts = vec![ex("x1", "Your son is in cancer.", &["sign:cancer"])];
        correct_in(&mut chart, "x1", "Your sun is in cancer.").unwrap();
        let ex = &chart.excerpts[0];
        assert_eq!(ex.text, "Your sun is in cancer.");
        assert!(ex.tags.contains(&"planet:sun".to_string()), "tags: {:?}", ex.tags);
        assert!(ex.tags.contains(&"sign:cancer".to_string()));
    }

    #[test]
    fn correction_without_router_hits_keeps_old_tags() {
        let mut chart = chart_fixture();
        chart.excerpts = vec![ex("x1", "Something vague.", &["house:2"])];
        correct_in(&mut chart, "x1", "Still nothing astrological here.").unwrap();
        assert_eq!(chart.excerpts[0].tags, vec!["house:2"]);
        assert!(correct_in(&mut chart, "x1", "   ").is_err());
    }
}
