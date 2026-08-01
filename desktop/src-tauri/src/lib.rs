//! Tauri backend: thin commands over the `astro` lib. The webview only
//! renders; every capability (ephemeris, gazetteer, whisper, routing,
//! emission) runs natively here, exactly as in the CLI/TUI.

mod library;
mod prefs;
#[cfg(desktop)]
mod record;
mod session;

use astro::chart::parse_time;
use astro::contract::{ChartData, Excerpt};
use astro::chart::systems;
use astro::route::{next_ordinal, retag};
use astro::{TranscriptSource, geo};
use serde::{Deserialize, Serialize};
use library::{Library, ReadingEntry};
use session::{Reading, Session};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

/// The backend's one piece of mutable state: the reading session. Every
/// command that needs a reading reaches it through [`Session`]'s guard, so the
/// "no chart has been built yet" refusal exists once (`session::NO_READING`)
/// rather than in each command that remembered to check.
#[derive(Default)]
struct AppState(Mutex<Session>);

impl AppState {
    fn session(&self) -> std::sync::MutexGuard<'_, Session> {
        self.0.lock().unwrap()
    }
}

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

/// The app's own window furniture, in the person's language.
///
/// The core has localized element names since the beginning, the PDF and the
/// artifact have had chrome tables for a while, and the window that produces
/// both was English-only. This serves the reading view's share of it; the forms
/// are still English in their components.
#[tauri::command]
fn app_chrome(app: AppHandle) -> &'static astro::i18n::AppChrome {
    // The person's language, not the reading's: an astrologer writing an
    // English reading still wants their own buttons.
    astro::i18n::Locale::parse(prefs::load(&app).default_locale.as_deref().unwrap_or("en")).app()
}

/// The calculation a form starts from when nothing has been chosen and nothing
/// is preferred — served so the webview stops restating the three codes.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
struct CalculationDefaults {
    house_system: String,
    zodiac: String,
    ayanamsa: String,
}

/// The three default codes. `"whole-sign"`, `"tropical"` and `"lahiri"` used to
/// be written out in five places on this side of the wire and five more on the
/// other; `chart::systems::DEFAULTS` is now the only one.
#[tauri::command]
fn calculation_defaults() -> CalculationDefaults {
    let d = systems::DEFAULTS;
    CalculationDefaults {
        house_system: d.house_system.expect("a default house system").to_string(),
        zodiac: d.zodiac.expect("a default zodiac").to_string(),
        ayanamsa: d.ayanamsa.expect("a default ayanamsa").to_string(),
    }
}

/// A calculation-option row for a UI selector: the stable wire `code` and its
/// display `label`. Serves both the house-system and ayanamsa dropdowns.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
struct OptionDto {
    code: String,
    label: String,
}

/// The house systems offered in the form, labelled in English (the canonical
/// system names). The chart itself records the label in the reading's locale.
#[tauri::command]
fn list_house_systems() -> Vec<OptionDto> {
    astro::chart::systems::HOUSE_SYSTEMS
        .iter()
        .map(|&(code, _)| OptionDto {
            code: code.to_string(),
            label: astro::i18n::Locale::En.house_system_label(code).to_string(),
        })
        .collect()
}

/// The ayanamsas offered when the sidereal zodiac is chosen, labelled by their
/// own names (proper nouns).
#[tauri::command]
fn list_ayanamsas() -> Vec<OptionDto> {
    astro::chart::systems::AYANAMSAS
        .iter()
        .map(|&(code, ayanamsa)| OptionDto { code: code.to_string(), label: ayanamsa.to_string() })
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
    /// House-system code ("whole-sign", "placidus", …); absent falls back to
    /// the default-house-system preference, then Whole Sign.
    house_system: Option<String>,
    /// Zodiac ("tropical" | "sidereal"); absent falls back to the preference,
    /// then tropical.
    zodiac: Option<String>,
    /// Ayanamsa code ("lahiri", …) used when `zodiac` is "sidereal"; absent
    /// falls back to the preference, then Lahiri.
    ayanamsa: Option<String>,
}

/// The shared tail of every chart-computing command (`build`, `reproject`,
/// `preview`): resolve the string-coded house-system/zodiac/ayanamsa choice
/// and assemble a computable input. Sidereal opts into an ayanamsa (Lahiri
/// unless named); any other zodiac string means tropical. The caller resolves
/// place and date itself — their failure messages differ per command.
fn resolve_input(
    name: &str,
    date: chrono::NaiveDate,
    time: &str,
    place: &geo::Place,
    locale: astro::i18n::Locale,
    asked: systems::Codes,
    preferred: systems::Codes,
) -> Result<astro::chart::BirthInput, String> {
    let calc = systems::resolve(asked, preferred)?;
    Ok(astro::birth_at_place(
        name,
        date,
        parse_time(time)?,
        place,
        locale,
        calc.house_system,
        calc.ayanamsa,
    ))
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
    // preference, else English. House system / zodiac / ayanamsa follow the
    // same ladder; the historical defaults (Whole Sign · Tropical · Lahiri)
    // live in `resolve_input`.
    let locale = astro::i18n::Locale::parse(
        form.lang.as_deref().or(p.default_locale.as_deref()).unwrap_or("en"),
    );
    let input = resolve_input(
        &form.name,
        date,
        &form.time,
        place,
        locale,
        systems::Codes::new(
            form.house_system.as_deref(),
            form.zodiac.as_deref(),
            form.ayanamsa.as_deref(),
        ),
        preferred(&p),
    )?;
    let source = TranscriptSource::classify(
        form.transcript.as_deref().unwrap_or(""),
        form.model.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;

    // The whole pipeline, on the blocking pool. `build_reading` hands back the
    // transcript it routed from (`report.transcript`), which is what the
    // readings library persists verbatim — the desktop used to fork this entire
    // match for want of that one value.
    let progress_app = app.clone();
    let (mut chart, report) = tauri::async_runtime::spawn_blocking(move || {
        astro::build_reading(&input, source, move |pct| {
            let _ = progress_app.emit("transcribe-progress", pct);
        })
    })
    .await
    .map_err(|e| format!("build task failed: {e}"))??;

    // Surface non-fatal warnings (DST-ambiguous birth time, Verify-gate
    // rejections) the pipeline used to write to stderr; the webview toasts them.
    if !report.warnings.is_empty() {
        let _ = app.emit("build-warnings", &report.warnings);
    }

    // Practitioner branding rides on the chart's meta (and thus into both
    // chart.json and the engraved artifact). Both best-effort.
    chart.meta.astrologer = p.astrologer.clone().filter(|s| !s.trim().is_empty());
    chart.meta.logo = p.logo.as_deref().and_then(|l| prefs::logo_data_uri(Path::new(l)));

    // The seed that lets the reading view recompute geometry live (house system
    // / zodiac). Rides into chart.json so reopened readings stay reprojectable.
    chart.meta.birth = Some(astro::contract::BirthSeed {
        place_id: form.place_id,
        date: form.date.clone(),
        time: form.time.clone(),
    });

    // Auto-save when a readings folder is configured; a reading without one is
    // held in the session and exported by hand.
    let stem = library::stem(&chart.meta.name, chrono::Local::now().date_naive());
    let session_dir = match Library::configured(p.readings_dir.as_deref()) {
        Some(lib) => Some(lib.create(&stem, &chart, report.transcript.as_ref())?),
        None => None,
    };

    state
        .session()
        .open(Reading::new(chart.clone(), session_dir, library::artifact_name(&stem)))?;
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
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let out = std::env::temp_dir().join(format!("astro-take-{millis}.wav"));
    // The session guards first and only then opens the device.
    state.session().begin_take(model, || {
        record::start(out).map(|r| Box::new(r) as Box<dyn session::Capture>)
    })
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
    // Stopping only ends the capture: the take is in flight until its words
    // land, and the session refuses to start another meanwhile.
    let pending = state.session().end_take()?;
    let (wav, model, locale) = (pending.wav.clone(), pending.model.clone(), pending.locale);

    let progress_app = app.clone();
    let transcribed = tauri::async_runtime::spawn_blocking(move || {
        astro::transcribe::transcribe(&wav, &model, Some(locale.whisper_lang()), move |pct| {
            let _ = progress_app.emit("transcribe-progress", pct);
        })
    })
    .await
    .map_err(|e| format!("transcription task failed: {e}"));

    // However it failed, the take does not land — put the reading back so the
    // astrologer can simply record again.
    let segments = match transcribed.and_then(|r| r) {
        Ok(segments) => segments,
        Err(e) => {
            state.session().abandon_take();
            return Err(e);
        }
    };

    let mut guard = state.session();
    let landed = guard.land_take(pending, segments)?;
    let reading = guard.reading()?;

    // library auto-save: the take's transcription (session-offset anchors,
    // matching the folio) and the refreshed chart
    if let Some(dir) = &reading.dir {
        let path = dir.join(&landed.filename);
        std::fs::write(&path, &landed.jsonl)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        library::save_chart(dir, &reading.chart)?;
    }
    let chart = reading.chart.clone();
    drop(guard);

    if !landed.warnings.is_empty() {
        let _ = app.emit("build-warnings", &landed.warnings);
    }
    Ok(chart)
}

/// The shared frame of every curation command: require a reading, mutate its
/// chart, refresh the library's chart.json, return the updated clone for the
/// webview. Deliberately available mid-take — curating while the recorder runs
/// is not refused, and the take appends to whatever it finds.
fn with_chart(
    state: &State<'_, AppState>,
    mutate: impl FnOnce(&mut ChartData) -> Result<(), String>,
) -> Result<ChartData, String> {
    let mut guard = state.session();
    let reading = guard.reading_mut()?;
    mutate(&mut reading.chart)?;
    if let Some(dir) = &reading.dir {
        library::save_chart(dir, &reading.chart)?;
    }
    Ok(reading.chart.clone())
}

/// Everything a reproject takes from the old chart. Only the geometry is
/// recomputed, so the identity that produced it (name, language, birth seed),
/// the routed passages, and the practitioner's branding all cross over
/// untouched — and the seed crosses too, or the *next* recalculation would find
/// nothing to recompute from.
///
/// Split out from the command so this can be tested: passages surviving a
/// house-system swap is the whole point of a reproject, and `State` cannot be
/// constructed in a test.
#[derive(Debug)]
struct Carried {
    name: String,
    locale: astro::i18n::Locale,
    seed: astro::contract::BirthSeed,
    excerpts: Vec<Excerpt>,
    astrologer: Option<String>,
    logo: Option<String>,
}

impl Carried {
    fn from(old: &ChartData) -> Result<Carried, String> {
        Ok(Carried {
            name: old.meta.name.clone(),
            locale: astro::i18n::Locale::parse(&old.meta.locale),
            seed: old
                .meta
                .birth
                .clone()
                .ok_or("this reading has no saved birth data to recalculate from")?,
            excerpts: old.excerpts.clone(),
            astrologer: old.meta.astrologer.clone(),
            logo: old.meta.logo.clone(),
        })
    }

    fn onto(self, chart: &mut ChartData) {
        chart.excerpts = self.excerpts;
        chart.meta.astrologer = self.astrologer;
        chart.meta.logo = self.logo;
        chart.meta.birth = Some(self.seed);
    }
}

/// Recompute the current chart's geometry under a new house system / zodiac,
/// live from the reading view. Reconstructs the birth input from `meta.birth`
/// (so it works for both freshly-built and library-reopened readings), then
/// carries the routed passages and branding onto the new chart — excerpt tags
/// describe spoken words, not placements, so they stay valid across the swap.
#[tauri::command]
fn reproject(
    state: State<'_, AppState>,
    house_system: String,
    zodiac: String,
    ayanamsa: Option<String>,
) -> Result<ChartData, String> {
    let mut guard = state.session();
    let carried = Carried::from(guard.chart()?)?;

    let place =
        geo::by_id(carried.seed.place_id).ok_or("the birth place is no longer in the gazetteer")?;
    let date =
        carried.seed.date.parse().map_err(|_| "the saved birth date is invalid".to_string())?;
    let input = resolve_input(
        &carried.name,
        date,
        &carried.seed.time.clone(),
        place,
        carried.locale,
        systems::Codes::new(Some(&house_system), Some(&zodiac), ayanamsa.as_deref()),
        systems::Codes::default(),
    )?;

    let mut chart = astro::chart::compute_chart(&input)?;
    carried.onto(&mut chart);

    let chart = guard.resettle(chart)?.clone();
    let reading = guard.reading()?;
    if let Some(dir) = &reading.dir {
        library::save_chart(dir, &reading.chart)?;
    }
    Ok(chart)
}

/// The calculation tier a person's stored preferences contribute.
fn preferred(p: &prefs::Preferences) -> systems::Codes<'_> {
    systems::Codes::new(
        p.default_house_system.as_deref(),
        p.default_zodiac.as_deref(),
        p.default_ayanamsa.as_deref(),
    )
}

/// A live-calculator moment: date/time/place plus calculation choices. No
/// name, transcript, or model — previews are anonymous geometry.
#[derive(Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
struct PreviewInput {
    date: String,
    time: String,
    place_id: u32,
    /// Locale for element labels; absent = English.
    lang: Option<String>,
    house_system: Option<String>,
    zodiac: Option<String>,
    ayanamsa: Option<String>,
}

/// A previewed chart with its non-fatal warnings inline. Unlike `build`, the
/// warnings are NOT emitted as an event — at scrub rates events would
/// toast-spam, so the calculator renders them as a quiet caption instead.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
struct PreviewDto {
    chart: ChartData,
    warnings: Vec<String>,
}

/// Chart geometry for an arbitrary moment — the live calculator's engine.
/// Side-effect-free by contract: reads no AppState, writes no files, touches
/// no session, so the frontend may call it at scrub rates. async keeps the
/// ephemeris math off the main thread.
#[tauri::command]
async fn preview(input: PreviewInput) -> Result<PreviewDto, String> {
    let locale = astro::i18n::Locale::parse(input.lang.as_deref().unwrap_or("en"));
    let place = geo::by_id(input.place_id).ok_or("pick a place from the suggestions")?;
    let date =
        input.date.parse().map_err(|_| "a date as YYYY-MM-DD, e.g. 1990-07-13".to_string())?;
    let birth = resolve_input(
        "", // anonymous → locale.anonymous()
        date,
        &input.time,
        place,
        locale,
        systems::Codes::new(
            input.house_system.as_deref(),
            input.zodiac.as_deref(),
            input.ayanamsa.as_deref(),
        ),
        // No preference tier: the calculator states every choice it has, and
        // seeds them from preferences itself at startup.
        systems::Codes::default(),
    )?;
    let (chart, warnings) = astro::chart::compute_chart_reporting(&birth)?;
    Ok(PreviewDto { chart, warnings })
}

/// The calculator's opening place: the persisted last-used place when it
/// still resolves in the gazetteer, else a default city. Total — the fallback
/// is a gazetteer *search*, so renumbered ids after a gazetteer rebuild can't
/// break first run.
#[tauri::command]
fn last_place(app: AppHandle) -> PlaceDto {
    prefs::load(&app)
        .last_place_id
        .and_then(geo::by_id)
        .or_else(|| geo::search("London", 1).into_iter().next())
        .map(|p| PlaceDto { id: p.id, label: p.label() })
        .expect("the gazetteer always resolves the default city")
}

/// Persist the calculator's place — a read-modify-write of preferences.json.
/// Called when a place is picked, never per scrub tick.
#[tauri::command]
fn set_last_place(app: AppHandle, id: u32) -> Result<(), String> {
    let mut p = prefs::load(&app);
    p.last_place_id = Some(id);
    prefs::save(&app, &p)
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

/// Open the bundled third-party license notices (Apache-2.0 attribution for the
/// `xalen-*` ephemeris crates and every other dependency) in the OS browser.
/// The file is generated by `cargo about` and shipped as an app resource.
#[tauri::command]
fn open_licenses(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    use tauri_plugin_opener::OpenerExt;
    let path = app
        .path()
        .resolve("THIRD-PARTY-LICENSES.html", tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("cannot locate the licenses file: {e}"))?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("cannot open the licenses file: {e}"))
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
        default_house_system: norm(prefs.default_house_system),
        default_zodiac: norm(prefs.default_zodiac),
        default_ayanamsa: norm(prefs.default_ayanamsa),
        // Not a pane field — carried through so saving preferences never
        // erases the calculator's remembered place.
        last_place_id: prefs.last_place_id,
    };
    if let Some(size) = &prefs.page_size {
        astro::pdf::PageSize::parse(size)?;
    }
    // The calculation preferences were the four this never checked, so a
    // nonsense value persisted and then quietly became Whole Sign on every
    // build. Resolving them here refuses at the point a person can still see
    // what they typed.
    //
    // The ladder only consults an ayanamsa under a sidereal zodiac, so this asks
    // for sidereal to have it checked, then checks the stored zodiac on its own.
    // Both go through `systems` — the hand-written comparison this replaced
    // trimmed and case-folded on one side only, so `" Tropical "` was refused
    // while `" Sidereal "` passed.
    systems::resolve(
        systems::Codes::default(),
        systems::Codes::new(
            prefs.default_house_system.as_deref(),
            Some("sidereal"),
            prefs.default_ayanamsa.as_deref(),
        ),
    )?;
    if let Some(z) = &prefs.default_zodiac {
        systems::is_sidereal(z)?;
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
    let mut chart: ChartData =
        serde_json::from_str(&raw).map_err(|e| format!("not a Midheaven chart.json: {e}"))?;
    // Derived fields first: charts saved before they existed carry none, and a
    // hand-edited file's copies are not to be trusted. Recomputing from the
    // longitudes makes both cases identical and gives `validate` real values to
    // check rather than whatever the file claimed.
    astro::derive::fill(&mut chart);
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
        .unwrap_or_else(|| library::stem(&chart.meta.name, chrono::Local::now().date_naive()));
    let takes = dir.as_deref().map(library::max_take_ordinal).unwrap_or(0);

    state
        .session()
        .open(Reading::reopened(chart.clone(), dir, library::artifact_name(&stem), takes))?;
    Ok(chart)
}

/// The readings library, newest first: every direct subfolder of the
/// configured readings dir that holds a `chart.json`. Empty when no readings
/// folder is set. Foreign or unreadable folders are silently skipped.
#[tauri::command]
fn list_readings(app: AppHandle) -> Vec<ReadingEntry> {
    Library::configured(prefs::load(&app).readings_dir.as_deref())
        .map(|lib| lib.entries())
        .unwrap_or_default()
}

/// Remove a reading from the library, folder and all. Guarded by
/// [`reading_to_remove`] so only a real reading inside the library root can go.
#[tauri::command]
fn delete_reading(app: AppHandle, dir: String) -> Result<(), String> {
    Library::configured(prefs::load(&app).readings_dir.as_deref())
        .ok_or("no readings folder configured")?
        .remove(&dir)
}

/// The generated export name, `{name}_{date}.html` — the save dialog's
/// default, matching the library folder convention.
#[tauri::command]
fn artifact_filename(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.session().reading()?.artifact_name.clone())
}

// async: rendering + disk write stay off the main thread. Both exporters clone
// the chart out of a scoped lock first — rendering under the lock would stall
// every other command for the length of a disk write.
#[tauri::command]
async fn save_artifact(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let chart = state.session().chart()?.clone();
    tauri::async_runtime::spawn_blocking(move || {
        astro::emit::write_artifact(&chart, path.as_ref()).map(|()| path)
    })
    .await
    .map_err(|e| format!("artifact task failed: {e}"))?
}

/// The PDF rendition; page size comes from preferences (A4
/// unless set to letter).
#[tauri::command]
async fn save_pdf(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<String, String> {
    let size = astro::pdf::PageSize::from_pref(prefs::load(&app).page_size.as_deref())?;
    let chart = state.session().chart()?.clone();
    tauri::async_runtime::spawn_blocking(move || {
        astro::pdf::write_pdf(&chart, size, path.as_ref()).map(|()| path)
    })
    .await
    .map_err(|e| format!("pdf task failed: {e}"))?
}

/// The command set, declared once.
///
/// Takes the macro to hand the list to, plus any target-specific extras, so the
/// names exist in exactly one place: [`run`] passes `handler` to register them,
/// and the `ts` build passes `names` to generate the `CommandName` union the
/// webview calls through. Renaming a command means renaming its entry here —
/// which will not compile until the function matches, and regenerates the union,
/// so the webview cannot keep calling the old name and pass CI.
///
/// It does not cover *argument* names: those are still matched by hand in
/// `api.ts` against each function's parameters.
macro_rules! commands {
    ($mac:ident $(, $extra:ident)*) => {
        $mac![
            search_places,
            list_locales,
            list_house_systems,
            list_ayanamsas,
            calculation_defaults,
            app_chrome,
            build,
            reproject,
            preview,
            last_place,
            set_last_place,
            save_artifact,
            save_pdf,
            merge_up,
            correct_excerpt,
            add_excerpt,
            delete_excerpt,
            get_preferences,
            set_preferences,
            open_licenses,
            list_models,
            artifact_filename,
            load_chart,
            list_readings,
            delete_reading
            $(, $extra)*
        ]
    };
}

/// Register the list with Tauri.
macro_rules! handler {
    ($($name:ident),* $(,)?) => { tauri::generate_handler![$($name),*] };
}

/// The list as strings, for the generated TypeScript.
#[cfg(feature = "ts")]
macro_rules! names {
    ($($name:ident),* $(,)?) => { [$(stringify!($name)),*] };
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

    // The recording pair only exists on desktop; `generate_handler!` can't take
    // `#[cfg]` on its entries, so each target passes its own extras to the one
    // shared list in `commands!`.
    #[cfg(desktop)]
    let builder = builder.invoke_handler(commands!(handler, start_recording, stop_recording));
    #[cfg(mobile)]
    let builder = builder.invoke_handler(commands!(handler));

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Generate the webview's `CommandName` union from the same list `run()`
/// registers, alongside the ts-rs DTO bindings.
///
/// A test rather than a build script because that is how the DTO bindings are
/// generated too — `npm run gen:types` (and CI) run both in one `cargo test
/// ... export_bindings`, and the committed output is diffed. Named to match that
/// filter.
#[cfg(all(test, feature = "ts"))]
#[test]
fn export_bindings_command_names() {
    // Same convention as ts-rs: the export dir comes from the environment, and
    // paths are relative to it. Without it there is nothing to write to.
    let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else {
        return;
    };
    // Length inferred — adding a command should not need a number bumped here.
    let names = commands!(names, start_recording, stop_recording);
    let mut out = String::from(
        "// This file was generated from the command list in \
         `desktop/src-tauri/src/lib.rs`. Do not edit this file manually.\n\n\
         /** Every command the backend registers. `api.ts` calls through this, so a\n\
         \x20* command renamed in Rust becomes a TypeScript error rather than a runtime\n\
         \x20* rejection. Includes the desktop-only recording pair, which a mobile build\n\
         \x20* does not register. */\nexport type CommandName =\n",
    );
    for name in names {
        out.push_str(&format!("  | \"{name}\"\n"));
    }
    out.push_str("  ;\n");

    let path = std::path::Path::new(&dir).join("generated").join("commands.ts");
    std::fs::create_dir_all(path.parent().expect("generated dir")).expect("create generated dir");
    std::fs::write(&path, out).expect("write commands.ts");
}

#[cfg(test)]
mod tests {
    use super::*;
    use astro::route::{Filing, route_into};

    fn chart_fixture() -> ChartData {
        astro::fixtures::berlin_chart()
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
    fn chart_json_round_trips_for_loading() {
        // The load_chart path relies on ChartData deserializing from the same
        // pretty JSON `library::save_chart` writes. Route a passage first so the
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

    /// A reproject changes only the geometry. Everything else — the routed
    /// passages, the practitioner's branding, and the birth seed that makes the
    /// *next* recalculation possible — must cross onto the recomputed chart.
    #[test]
    fn a_reproject_carries_passages_branding_and_the_seed_onto_the_new_geometry() {
        let mut old = chart_fixture();
        old.meta.birth = Some(astro::contract::BirthSeed {
            place_id: 2950159,
            date: "1990-07-13".into(),
            time: "14:30".into(),
        });
        old.meta.astrologer = Some("A. Practitioner".into());
        old.meta.logo = Some("data:image/png;base64,AAAA".into());
        old.excerpts = vec![ex("x1", "The sun in cancer.", &["planet:sun", "sign:cancer"])];

        let carried = Carried::from(&old).expect("a seeded chart can be recalculated");
        assert_eq!(carried.name, old.meta.name);
        assert_eq!(carried.locale, astro::i18n::Locale::En);

        // The recomputed chart arrives with fresh geometry and nothing else.
        let mut fresh = {
            let mut input = astro::fixtures::berlin();
            input.house_system = systems::house_system("placidus").unwrap();
            astro::chart::compute_chart(&input).unwrap()
        };
        assert!(fresh.excerpts.is_empty());
        assert_ne!(fresh.house_cusps, old.house_cusps, "the geometry really changed");

        carried.onto(&mut fresh);

        assert_eq!(fresh.excerpts.len(), 1);
        assert_eq!(fresh.excerpts[0].text, "The sun in cancer.");
        assert_eq!(fresh.meta.astrologer.as_deref(), Some("A. Practitioner"));
        assert_eq!(fresh.meta.logo.as_deref(), Some("data:image/png;base64,AAAA"));
        assert!(fresh.meta.birth.is_some(), "still recalculable afterwards");
        // The carried tags remain in the new chart's vocabulary — that is what
        // makes carrying them legitimate rather than a leak.
        assert!(fresh.validate().is_ok(), "{:?}", fresh.validate());
    }

    /// A chart with no birth seed (CLI output, or one saved before seeds
    /// existed) simply cannot be recalculated, and says so.
    #[test]
    fn a_reproject_refuses_a_chart_with_no_birth_seed() {
        let mut old = chart_fixture();
        old.meta.birth = None;
        assert_eq!(
            Carried::from(&old).unwrap_err(),
            "this reading has no saved birth data to recalculate from"
        );
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
    fn resolve_input_carries_the_calculation_and_gates_the_time() {
        let place = geo::search("Berlin", 1).into_iter().next().expect("gazetteer has Berlin");
        let date: chrono::NaiveDate = "1990-07-13".parse().unwrap();
        let en = astro::i18n::Locale::En;
        let none = systems::Codes::default();
        let asked = |h, z, a| systems::Codes::new(Some(h), Some(z), a);

        // The ladder itself is tested in `chart::systems`; what this checks is
        // that the command layer hands it through to the birth input.
        let sid = resolve_input("", date, "14:30", place, en, asked("placidus", "Sidereal", None), none)
            .unwrap();
        assert_eq!(sid.house_system, systems::house_system("placidus").unwrap());
        assert_eq!(sid.ayanamsa, Some(systems::ayanamsa("lahiri").unwrap()));
        // A blank name resolves to the locale's anonymous label.
        assert!(!sid.name.is_empty());

        // A preference tier reaches it too.
        let pref = resolve_input("", date, "14:30", place, en, none, asked("koch", "tropical", None))
            .unwrap();
        assert_eq!(pref.house_system, systems::house_system("koch").unwrap());
        assert_eq!(pref.ayanamsa, None);

        // The shared time rule still gates: nonsense fails here, not deeper.
        assert!(
            resolve_input("", date, "25:00", place, en, none, none).is_err(),
            "an impossible time is refused"
        );
        // And so does an impossible calculation.
        assert!(resolve_input("", date, "14:30", place, en, asked("bogus", "tropical", None), none)
            .is_err());
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
        route_into(&mut chart, &take, Filing::Append);
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
