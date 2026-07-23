//! Tauri backend: thin commands over the `astro` lib. The webview only
//! renders; every capability (ephemeris, gazetteer, whisper, routing,
//! emission) runs natively here, exactly as in the CLI/TUI.

mod prefs;
#[cfg(desktop)]
mod record;

use astro::chart::parse_time;
use astro::contract::{ChartData, Excerpt};
use astro::route::{Transcript, index_transcript, lexicon_for, next_ordinal, retag};
#[cfg(desktop)]
use astro::route::append_transcript;
use astro::{TranscriptSource, geo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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
    /// Total seconds recorded this session — offsets each new take's
    /// timestamps so folio anchors run continuously.
    session_secs: f64,
    #[cfg(desktop)]
    model: Option<PathBuf>,
    #[cfg(desktop)]
    recorder: Option<record::Recorder>,
    /// `{readings_dir}/{name}_{date}/` when a readings folder is configured —
    /// chart.json and transcriptions auto-save here through the session.
    session_dir: Option<PathBuf>,
    /// Suggested export name, `{name}_{date}.html` — set at build.
    artifact_name: String,
    /// Live takes persisted this session (numbers `take-{n}.jsonl`).
    takes: usize,
}

/// Filesystem-safe name stem: lowercase, runs of anything non-alphanumeric
/// collapse to one `_`. The library folder is `{slug}_{YYYY-MM-DD}`.
fn slug(name: &str) -> String {
    let parts: Vec<String> = name
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();
    if parts.is_empty() { "reading".to_string() } else { parts.join("_") }
}

fn save_chart_json(dir: &Path, chart: &ChartData) -> Result<(), String> {
    let path = dir.join("chart.json");
    let json = serde_json::to_string_pretty(chart).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Highest `n` among the `take-{n}.jsonl` files already in a reading folder, so
/// a take recorded after reopening never overwrites one. 0 when none exist or
/// the folder can't be read.
fn max_take_ordinal(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter_map(|n| {
                    n.strip_prefix("take-")
                        .and_then(|r| r.strip_suffix(".jsonl"))
                        .and_then(|d| d.parse::<usize>().ok())
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

#[derive(Default)]
struct AppState(Mutex<Inner>);

/// Just enough for the typeahead: the id round-trips to `geo::by_id` at
/// build time — coordinates and zone stay backend-side.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
struct PlaceDto {
    id: u32,
    label: String,
}

/// A reading language for the UI selectors, sourced from `i18n` so the
/// frontend never re-encodes the language list, endonyms, or the house-name
/// suffix (`list_locales`).
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
struct LocaleDto {
    /// Short code persisted on `meta.locale` (`en`, `ru`).
    code: String,
    /// The language's own name (endonym), shown in the selector.
    label: String,
    /// Word to strip from a house name to show the bare ordinal ("First").
    house_suffix: String,
}

/// One row of the readings library: enough to list and reopen a saved
/// reading without the frontend touching the filesystem.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
struct ReadingEntry {
    /// `{dir}/chart.json` — fed straight to `load_chart`.
    chart_path: String,
    /// The reading's folder — fed to `delete_reading`.
    dir: String,
    name: String,
    born: String,
    place: String,
    excerpts: usize,
    /// `chart.json`'s mtime, ms since the epoch — sort key and "saved" date.
    /// Serialized as a JSON number; ms-since-epoch stays within JS's safe
    /// integer range for millennia, so the binding is `number`, not `bigint`.
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    modified_ms: Option<u64>,
}

/// Read a library folder's `chart.json` into a listing row. `None` (skipped
/// from the list) when the folder holds no chart or an unreadable one.
fn reading_entry(dir: &Path) -> Option<ReadingEntry> {
    let chart_path = dir.join("chart.json");
    let raw = std::fs::read_to_string(&chart_path).ok()?;
    let chart: ChartData = serde_json::from_str(&raw).ok()?;
    let modified_ms = std::fs::metadata(&chart_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    Some(ReadingEntry {
        chart_path: chart_path.to_string_lossy().into_owned(),
        dir: dir.to_string_lossy().into_owned(),
        name: chart.meta.name,
        born: chart.meta.born,
        place: chart.meta.place,
        excerpts: chart.excerpts.len(),
        modified_ms,
    })
}

/// Resolve a delete target safely: it must canonicalize to a *direct* child of
/// the library root and actually be a saved reading (contain `chart.json`), so
/// no path outside the library or non-reading folder can be removed.
fn reading_to_remove(root: &Path, dir: &str) -> Result<PathBuf, String> {
    let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let target = std::fs::canonicalize(dir).map_err(|e| format!("no folder at {dir}: {e}"))?;
    if target.parent() != Some(root.as_path()) {
        return Err("that folder is not in the readings library".to_string());
    }
    if !target.join("chart.json").is_file() {
        return Err("that folder is not a saved reading".to_string());
    }
    Ok(target)
}

// async: keeps the gazetteer scan (and a possible cold-parse stall on the
// very first keystroke) off the main thread
#[tauri::command]
async fn search_places(query: String) -> Vec<PlaceDto> {
    geo::search(&query, 6)
        .into_iter()
        .map(|p| PlaceDto { id: p.id, label: p.label() })
        .collect()
}

/// The reading languages offered in the UI, each with its endonym and the
/// house-name suffix to strip — the single source the frontend builds its
/// language selector and house labels from (see `i18n::Locale`).
#[tauri::command]
fn list_locales() -> Vec<LocaleDto> {
    astro::i18n::Locale::ALL
        .iter()
        .map(|&l| LocaleDto {
            code: l.code().to_string(),
            label: l.endonym().to_string(),
            house_suffix: l.house_suffix().to_string(),
        })
        .collect()
}

#[derive(Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
struct BirthForm {
    name: String,
    date: String,
    time: String,
    place_id: u32,
    transcript: Option<String>,
    model: Option<String>,
    /// Reading language code ("en", "ru"); absent falls back to the
    /// default-language preference, then English.
    lang: Option<String>,
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

    let p = prefs::load(&app);
    // Per-reading language: the form's choice, else the default-language
    // preference, else English.
    let locale = astro::i18n::Locale::parse(
        form.lang.as_deref().or(p.default_locale.as_deref()).unwrap_or("en"),
    );
    let input = astro::birth_at_place(&form.name, date, parse_time(&form.time)?, place, locale);
    let source = TranscriptSource::classify(
        form.transcript.as_deref().unwrap_or(""),
        form.model.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;

    // Unlike `build_reading` (the CLI/TUI path), the desktop keeps the
    // transcript at hand so the readings library can persist it verbatim.
    // Only the audio arm reports progress; on mobile that arm is compiled out.
    #[cfg(desktop)]
    let progress_app = app.clone();
    type Persisted = Option<(String, String)>; // (filename, contents) for the library
    let (mut chart, transcript_file) = tauri::async_runtime::spawn_blocking(
        move || -> Result<(ChartData, Persisted), String> {
            let (transcript, persisted): (Option<Transcript>, Persisted) = match source {
                TranscriptSource::None => (None, None),
                TranscriptSource::File(path) => {
                    let raw = std::fs::read_to_string(&path)
                        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("txt");
                    (Some(Transcript::load(&raw)), Some((format!("transcript.{ext}"), raw)))
                }
                #[cfg(desktop)]
                TranscriptSource::Audio { wav, model } => {
                    let segments =
                        astro::transcribe::transcribe(&wav, &model, Some(locale.whisper_lang()), move |pct| {
                            let _ = progress_app.emit("transcribe-progress", pct);
                        })?;
                    let jsonl = astro::transcribe::to_jsonl(&segments);
                    (
                        Some(Transcript::from_segments(segments)),
                        Some(("transcript.jsonl".to_string(), jsonl)),
                    )
                }
            };
            let mut chart = astro::chart::compute_chart(&input)?;
            if let Some(t) = &transcript {
                let router = lexicon_for(&chart);
                index_transcript(&mut chart, t, &router);
            }
            Ok((chart, persisted))
        },
    )
    .await
    .map_err(|e| format!("build task failed: {e}"))??;

    // Practitioner branding rides on the chart's meta (and thus into both
    // chart.json and the engraved artifact). Both best-effort.
    chart.meta.astrologer = p.astrologer.clone().filter(|s| !s.trim().is_empty());
    chart.meta.logo = p.logo.as_deref().and_then(|l| prefs::logo_data_uri(Path::new(l)));

    let stem = format!("{}_{}", slug(&chart.meta.name), chrono::Local::now().format("%Y-%m-%d"));
    let session_dir = match p.readings_dir.as_deref().map(str::trim) {
        Some(root) if !root.is_empty() => {
            let dir = PathBuf::from(root).join(&stem);
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            if let Some((name, contents)) = &transcript_file {
                std::fs::write(dir.join(name), contents)
                    .map_err(|e| format!("cannot write {name}: {e}"))?;
            }
            save_chart_json(&dir, &chart)?;
            Some(dir)
        }
        _ => None,
    };

    let mut inner = state.0.lock().unwrap();
    inner.session_secs = 0.0;
    inner.takes = 0;
    inner.session_dir = session_dir;
    inner.artifact_name = format!("{stem}.html");
    inner.chart = Some(chart.clone());
    drop(inner);
    Ok(chart)
}

/// Begin capturing the session from the default microphone. The model path
/// comes from the form (the frontend only shows the button when it is set).
/// Desktop-only: mobile builds ship no on-device recording/transcription.
#[cfg(desktop)]
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
/// Desktop-only: mobile builds ship no on-device recording/transcription.
#[cfg(desktop)]
#[tauri::command]
async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ChartData, String> {
    let (recorder, model, offset, locale) = {
        let mut inner = state.0.lock().unwrap();
        // Transcribe the take in the chart's own language.
        let locale = inner
            .chart
            .as_ref()
            .map(|c| astro::i18n::Locale::parse(&c.meta.locale))
            .unwrap_or_default();
        (
            inner.recorder.take().ok_or("not recording")?,
            inner.model.clone().ok_or("no model on record")?,
            inner.session_secs,
            locale,
        )
    };
    let (wav, secs) = recorder.stop()?;

    let progress_app = app.clone();
    let mut segments = tauri::async_runtime::spawn_blocking(move || {
        astro::transcribe::transcribe(&wav, &model, Some(locale.whisper_lang()), move |pct| {
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
    inner.session_secs = offset + secs;
    let chart = inner.chart.as_mut().ok_or("no chart has been built yet")?;

    // Route ONLY the new take and append, so earlier curation (merges,
    // corrections) survives every stop.
    let jsonl = astro::transcribe::to_jsonl(&segments);
    let take = Transcript::from_segments(segments);
    append_transcript(chart, &take, &lexicon_for(chart));

    // library auto-save: the take's transcription (session-offset anchors,
    // matching the folio) and the refreshed chart
    if let Some(dir) = &inner.session_dir {
        inner.takes += 1;
        let path = dir.join(format!("take-{}.jsonl", inner.takes));
        std::fs::write(&path, jsonl).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        save_chart_json(dir, chart)?;
    }
    Ok(chart.clone())
}

/// The shared frame of every curation command: lock, require a chart,
/// mutate, refresh the library's chart.json, return the updated clone for
/// the webview.
fn with_chart(
    state: &State<'_, AppState>,
    mutate: impl FnOnce(&mut ChartData) -> Result<(), String>,
) -> Result<ChartData, String> {
    let mut guard = state.0.lock().unwrap();
    let inner = &mut *guard;
    let chart = inner.chart.as_mut().ok_or("no chart has been built yet")?;
    mutate(chart)?;
    if let Some(dir) = &inner.session_dir {
        save_chart_json(dir, chart)?;
    }
    Ok(chart.clone())
}

/// Merge the excerpt into its predecessor: verbatim parts joined, tags
/// unioned, the earlier passage's time anchor kept (contract semantics via
/// [`Excerpt::absorb`]; only the text-joining strategy is ours).
fn merge_up_in(excerpts: &mut Vec<Excerpt>, id: &str) -> Result<(), String> {
    let i = excerpts.iter().position(|e| e.id == id).ok_or("no such passage")?;
    if i == 0 {
        return Err("the first passage has nothing above it to merge into".to_string());
    }
    let cur = excerpts.remove(i);
    let joined = format!("{} {}", excerpts[i - 1].text, cur.text);
    excerpts[i - 1].absorb(cur);
    excerpts[i - 1].text = joined;
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
    let tags = retag(chart, text); // same gated path as all routing
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
    with_chart(&state, |chart| merge_up_in(&mut chart.excerpts, &id))
}

#[tauri::command]
fn correct_excerpt(
    state: State<'_, AppState>,
    id: String,
    text: String,
) -> Result<ChartData, String> {
    with_chart(&state, |chart| correct_in(chart, &id, &text))
}

/// Author a passage by hand. Hand-picked tags must exist in the chart's
/// vocabulary (the verify gate's spirit); with none picked, the router files
/// it from the words — and a passage it can't file stays untagged, visible
/// whenever no filter is active.
fn add_in(chart: &mut ChartData, text: &str, tags: Vec<String>) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("a passage needs words".to_string());
    }
    let vocab = chart.vocab();
    if let Some(bad) = tags.iter().find(|t| !vocab.contains(*t)) {
        return Err(format!("unknown tag {bad}"));
    }
    let tags = if tags.is_empty() {
        retag(chart, text) // already sorted + deduped
    } else {
        let mut tags = tags;
        tags.sort();
        tags.dedup();
        tags
    };
    chart.excerpts.push(Excerpt {
        id: format!("x{}", next_ordinal(&chart.excerpts)),
        time: String::new(),
        span: [0, 0], // authored, not anchored to a transcript
        text: text.to_string(),
        tags,
    });
    Ok(())
}

#[tauri::command]
fn add_excerpt(
    state: State<'_, AppState>,
    text: String,
    tags: Vec<String>,
) -> Result<ChartData, String> {
    with_chart(&state, |chart| add_in(chart, &text, tags))
}

/// Remove a passage. The frontend confirms first; removal is final.
fn delete_in(excerpts: &mut Vec<Excerpt>, id: &str) -> Result<(), String> {
    let i = excerpts.iter().position(|e| e.id == id).ok_or("no such passage")?;
    excerpts.remove(i);
    Ok(())
}

#[tauri::command]
fn delete_excerpt(state: State<'_, AppState>, id: String) -> Result<ChartData, String> {
    with_chart(&state, |chart| delete_in(&mut chart.excerpts, &id))
}

#[tauri::command]
fn get_preferences(app: AppHandle) -> prefs::Preferences {
    prefs::load(&app)
}

/// Persist preferences, normalizing blanks to None and refusing paths that
/// don't exist — a bad folder should fail here, not at the next build.
#[tauri::command]
fn set_preferences(app: AppHandle, prefs: prefs::Preferences) -> Result<(), String> {
    let norm = |o: Option<String>| {
        o.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    };
    let prefs = prefs::Preferences {
        models_dir: norm(prefs.models_dir),
        default_model: norm(prefs.default_model),
        readings_dir: norm(prefs.readings_dir),
        astrologer: norm(prefs.astrologer),
        logo: norm(prefs.logo),
        page_size: norm(prefs.page_size),
        default_locale: norm(prefs.default_locale),
    };
    if let Some(size) = &prefs.page_size {
        astro::pdf::PageSize::parse(size)?;
    }
    for (label, dir) in [("models folder", &prefs.models_dir), ("readings folder", &prefs.readings_dir)] {
        if let Some(d) = dir {
            if !Path::new(d).is_dir() {
                return Err(format!("{label}: no folder at {d}"));
            }
        }
    }
    for (label, file) in [("default model", &prefs.default_model), ("logo", &prefs.logo)] {
        if let Some(f) = file {
            if !Path::new(f).is_file() {
                return Err(format!("{label}: no file at {f}"));
            }
        }
    }
    prefs::save(&app, &prefs)
}

/// Full paths of the ggml models (`.bin`) in a folder, sorted — feeds the
/// preferences pane's default-model picker.
#[tauri::command]
fn list_models(dir: String) -> Vec<String> {
    let mut models: Vec<String> = std::fs::read_dir(dir.trim())
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("bin"))
                .filter_map(|p| p.to_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    models.sort();
    models
}

/// Reopen a saved reading: parse a library `chart.json` back into the session
/// so it renders, curates, and re-exports exactly like a fresh build. The
/// file's own folder becomes the session dir (curation re-saves there, the
/// library convention), the folder name the suggested export stem, and new
/// live takes continue past any already on disk.
#[tauri::command]
fn load_chart(state: State<'_, AppState>, path: String) -> Result<ChartData, String> {
    let path = PathBuf::from(path.trim());
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let chart: ChartData =
        serde_json::from_str(&raw).map_err(|e| format!("not a Midheaven chart.json: {e}"))?;
    // The file is untrusted: enforce the structural/vocabulary invariants the
    // compute path guarantees but deserialization doesn't, before it can reach
    // curation, PDF export, or the emitted artifact.
    chart.validate().map_err(|e| format!("not a valid Midheaven chart.json: {e}"))?;

    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).map(Path::to_path_buf);
    let stem = dir
        .as_ref()
        .and_then(|d| d.file_name())
        .and_then(|s| s.to_str())
        .map(String::from)
        .unwrap_or_else(|| {
            format!("{}_{}", slug(&chart.meta.name), chrono::Local::now().format("%Y-%m-%d"))
        });
    let takes = dir.as_deref().map(max_take_ordinal).unwrap_or(0);

    let mut inner = state.0.lock().unwrap();
    inner.session_secs = 0.0;
    inner.takes = takes;
    inner.session_dir = dir;
    inner.artifact_name = format!("{stem}.html");
    inner.chart = Some(chart.clone());
    drop(inner);
    Ok(chart)
}

/// The readings library, newest first: every direct subfolder of the
/// configured readings dir that holds a `chart.json`. Empty when no readings
/// folder is set. Foreign or unreadable folders are silently skipped.
#[tauri::command]
fn list_readings(app: AppHandle) -> Vec<ReadingEntry> {
    let Some(root) = prefs::load(&app).readings_dir else {
        return Vec::new();
    };
    let mut entries: Vec<ReadingEntry> = std::fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter_map(|p| reading_entry(&p))
                .collect()
        })
        .unwrap_or_default();
    // newest first; entries without an mtime sink to the end
    entries.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    entries
}

/// Remove a reading from the library, folder and all. Guarded by
/// [`reading_to_remove`] so only a real reading inside the library root can go.
#[tauri::command]
fn delete_reading(app: AppHandle, dir: String) -> Result<(), String> {
    let root = prefs::load(&app).readings_dir.ok_or("no readings folder configured")?;
    let target = reading_to_remove(Path::new(&root), &dir)?;
    std::fs::remove_dir_all(&target)
        .map_err(|e| format!("cannot remove {}: {e}", target.display()))
}

/// The generated export name, `{name}_{date}.html` — the save dialog's
/// default, matching the library folder convention.
#[tauri::command]
fn artifact_filename(state: State<'_, AppState>) -> Result<String, String> {
    let inner = state.0.lock().unwrap();
    if inner.chart.is_none() {
        return Err("no chart has been built yet".to_string());
    }
    Ok(inner.artifact_name.clone())
}

// async: rendering + disk write stay off the main thread
#[tauri::command]
async fn save_artifact(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let guard = state.0.lock().unwrap();
    let chart = guard.chart.as_ref().ok_or("no chart has been built yet")?;
    astro::emit::write_artifact(chart, path.as_ref())?;
    Ok(path)
}

/// The PDF rendition; page size comes from preferences (A4
/// unless set to letter).
#[tauri::command]
async fn save_pdf(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<String, String> {
    let size = astro::pdf::PageSize::from_pref(prefs::load(&app).page_size.as_deref())?;
    let chart = {
        let guard = state.0.lock().unwrap();
        guard.chart.as_ref().ok_or("no chart has been built yet")?.clone()
    };
    tauri::async_runtime::spawn_blocking(move || {
        astro::pdf::write_pdf(&chart, size, path.as_ref()).map(|()| path)
    })
    .await
    .map_err(|e| format!("pdf task failed: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|_app| {
            // one-time gazetteer parse off the critical path
            tauri::async_runtime::spawn_blocking(geo::warm);
            Ok(())
        });

    // The recording commands only exist on desktop; `generate_handler!` can't
    // take `#[cfg]` on its entries, so the handler list is registered per target.
    #[cfg(desktop)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        search_places,
        list_locales,
        build,
        save_artifact,
        save_pdf,
        start_recording,
        stop_recording,
        merge_up,
        correct_excerpt,
        add_excerpt,
        delete_excerpt,
        get_preferences,
        set_preferences,
        list_models,
        artifact_filename,
        load_chart,
        list_readings,
        delete_reading
    ]);
    #[cfg(mobile)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        search_places,
        list_locales,
        build,
        save_artifact,
        save_pdf,
        merge_up,
        correct_excerpt,
        add_excerpt,
        delete_excerpt,
        get_preferences,
        set_preferences,
        list_models,
        artifact_filename,
        load_chart,
        list_readings,
        delete_reading
    ]);

    builder
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
            locale: astro::i18n::Locale::En,
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
    fn reading_to_remove_only_accepts_readings_inside_the_library() {
        let base = std::env::temp_dir().join("astro-lib-remove-test");
        std::fs::remove_dir_all(&base).ok(); // clean any prior run
        let root = base.join("readings");
        let reading = root.join("mira_2026-07-18");
        std::fs::create_dir_all(&reading).unwrap();
        std::fs::write(reading.join("chart.json"), "{}").unwrap();
        let stray = root.join("notes"); // a folder, but no chart.json
        std::fs::create_dir_all(&stray).unwrap();
        let outside = base.join("elsewhere"); // not a child of root
        std::fs::create_dir_all(&outside).unwrap();

        assert!(reading_to_remove(&root, reading.to_str().unwrap()).is_ok());
        assert!(reading_to_remove(&root, stray.to_str().unwrap()).is_err());
        assert!(reading_to_remove(&root, outside.to_str().unwrap()).is_err());
        assert!(reading_to_remove(&root, root.join("ghost").to_str().unwrap()).is_err());

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn chart_json_round_trips_for_loading() {
        // The load_chart path relies on ChartData deserializing from the same
        // pretty JSON `save_chart_json` writes. Route a passage first so the
        // excerpt list is non-empty.
        let mut chart = chart_fixture();
        chart.excerpts = vec![ex("x1", "The sun in cancer.", &["planet:sun", "sign:cancer"])];
        let json = serde_json::to_string_pretty(&chart).unwrap();
        let back: ChartData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.meta.name, chart.meta.name);
        assert_eq!(back.planets.len(), chart.planets.len());
        assert_eq!(back.aspects.len(), chart.aspects.len());
        assert_eq!(back.excerpts.len(), 1);
        assert_eq!(back.excerpts[0].text, "The sun in cancer.");
        assert_eq!(back.excerpts[0].tags, vec!["planet:sun", "sign:cancer"]);
        assert_eq!(back.excerpts[0].span, [0, "The sun in cancer.".len()]);
        // `Aspect::kind` is #[serde(skip)] — it defaults to "" on load.
        assert!(back.aspects.iter().all(|a| a.kind.is_empty()));
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
    fn added_passage_validates_tags_and_continues_ids_past_gaps() {
        let mut chart = chart_fixture();
        // merged lists leave gaps: x1, x5
        chart.excerpts = vec![
            ex("x1", "First.", &["planet:sun"]),
            ex("x5", "Fifth.", &["planet:moon"]),
        ];
        assert!(add_in(&mut chart, "Note.", vec!["planet:vulcan".into()]).is_err());
        add_in(&mut chart, "A note on the moon.", vec!["planet:moon".into()]).unwrap();
        let added = chart.excerpts.last().unwrap();
        assert_eq!(added.id, "x6");
        assert_eq!(added.tags, vec!["planet:moon"]);
        assert!(add_in(&mut chart, "   ", vec![]).is_err());
    }

    #[test]
    fn slug_collapses_to_filesystem_safe_stems() {
        assert_eq!(slug("Mira Holt"), "mira_holt");
        assert_eq!(slug("  Ana-María d'Été  "), "ana_maría_d_été");
        assert_eq!(slug("···"), "reading");
        assert_eq!(slug(""), "reading");
    }

    #[test]
    fn delete_removes_by_id_and_rejects_unknown() {
        let mut list = vec![ex("x1", "One.", &["planet:sun"]), ex("x2", "Two.", &["planet:moon"])];
        delete_in(&mut list, "x1").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "x2");
        assert!(delete_in(&mut list, "x9").is_err());
    }

    #[test]
    fn appended_takes_never_collide_with_added_passage_ids() {
        // add past a merge gap, then route a take: ids must stay unique
        let mut chart = chart_fixture();
        chart.excerpts = vec![ex("x1", "First.", &["planet:sun"]), ex("x5", "Fifth.", &["planet:moon"])];
        add_in(&mut chart, "A note.", vec!["planet:sun".into()]).unwrap(); // x6
        let take = astro::route::Transcript::from_segments([astro::contract::Segment {
            start: 0.0,
            text: "The moon in pisces.".into(),
        }]);
        let router = lexicon_for(&chart);
        astro::route::append_transcript(&mut chart, &take, &router);
        let mut ids: Vec<&str> = chart.excerpts.iter().map(|e| e.id.as_str()).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate excerpt ids: {ids:?}");
    }

    #[test]
    fn added_passage_without_tags_is_filed_by_the_router() {
        let mut chart = chart_fixture();
        add_in(&mut chart, "The sun rules this whole chart.", vec![]).unwrap();
        let added = chart.excerpts.last().unwrap();
        assert!(added.tags.contains(&"planet:sun".to_string()), "tags: {:?}", added.tags);
        assert_eq!(added.span, [0, 0]);
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
