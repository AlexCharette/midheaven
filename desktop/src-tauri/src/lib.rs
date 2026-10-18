//! Tauri backend: thin commands over the `astro` lib. The webview only
//! renders; every capability (ephemeris, gazetteer, whisper, routing,
//! emission) runs natively here, exactly as in the CLI/TUI.

mod ids;
mod naming;
mod notes;
mod prefs;
mod query;
#[cfg(desktop)]
mod record;
mod session;
mod store;

use astro::chart::parse_time;
use astro::contract::{ChartData, Note};
use astro::chart::systems;
use crate::store::{Chart, Person, Store, StoredNote};
use astro::{TranscriptSource, geo};
use serde::{Deserialize, Serialize};
use session::{Reading, Session};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

/// Milliseconds since the epoch. Ids are minted from it; every caller that
/// needs one goes through here rather than restating the `UNIX_EPOCH` dance.
fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

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
    let (mut chart, routed, report) = tauri::async_runtime::spawn_blocking(move || {
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

    // The person and the sky become records, and the take's notes are filed
    // under the sky. Without a configured folder there is nowhere to put any
    // of it, so the reading is held in the session and exported by hand.
    let chart_id = match store_of(&app) {
        Ok(store) => {
            let now = now_ms();
            let person = Person {
                id: store.mint_id::<Person>(now),
                name: chart.meta.name.clone(),
                born: chart.meta.born.clone(),
            };
            store.put(&person)?;
            let record = Chart {
                id: store.mint_id::<Chart>(now),
                kind: "natal".to_string(),
                seed: chart.meta.birth.clone().expect("stamped just above"),
                place: chart.meta.place.clone(),
                house_system: chart.meta.house_system.clone(),
                ayanamsa: chart.meta.ayanamsa.clone(),
                locale: chart.meta.locale.clone(),
                person: Some(person.id),
            };
            store.put(&record)?;
            chart.meta.id = Some(record.id.clone());
            notes::file_take(&store, &record.id, routed, now as i64)?;
            record.id
        }
        Err(_) => String::new(),
    };

    state.session().open(Reading::new(chart.clone(), chart_id))?;
    Ok(chart)
}

/// The store, or the refusal that means "no folder has been chosen yet".
fn store_of(app: &AppHandle) -> Result<Store, String> {
    Store::configured(prefs::load(app).readings_dir.as_deref())
        .ok_or_else(|| "choose a folder to keep your notes in first".to_string())
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
) -> Result<Vec<Note>, String> {
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
    let chart_id = guard.reading()?.chart_id.clone();
    drop(guard);

    // The take's notes are filed under the chart it was recorded into.
    let store = store_of(&app)?;
    notes::file_take(&store, &chart_id, landed.notes, now_ms() as i64)?;

    if !landed.warnings.is_empty() {
        let _ = app.emit("build-warnings", &landed.warnings);
    }
    Ok(notes::under(&store, &chart_id).iter().map(StoredNote::note).collect())
}

/// The chart a note command files against, or the one refusal.
///
/// Deliberately available mid-take: curating while the recorder runs is not
/// refused, and a landing take only ever adds to what it finds.
fn open_chart_id(state: &State<'_, AppState>) -> Result<String, String> {
    Ok(state.session().reading()?.chart_id.clone())
}

/// Everything a reproject takes from the old chart. Only the geometry is
/// recomputed, so the identity that produced it (name, language, birth seed)
/// and the practitioner's branding cross over untouched — and the seed
/// crosses too, or the *next* recalculation would find nothing to recompute
/// from.
///
/// The notes used to cross over here as well, because the chart carried them.
/// They no longer need to: they name the chart's id, and a reproject keeps
/// the id. A note's tags describe words rather than placements, so they stay
/// valid across the swap either way.
#[derive(Debug)]
struct Carried {
    name: String,
    locale: astro::i18n::Locale,
    seed: astro::contract::BirthSeed,
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
            astrologer: old.meta.astrologer.clone(),
            logo: old.meta.logo.clone(),
        })
    }

    fn onto(self, chart: &mut ChartData) {
        chart.meta.astrologer = self.astrologer;
        chart.meta.logo = self.logo;
        chart.meta.birth = Some(self.seed);
    }
}

/// Recompute the current chart's geometry under a new house system / zodiac,
/// live from the reading view. Reconstructs the birth input from `meta.birth`,
/// then carries the branding onto the new chart.
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

    let id = guard.reading()?.chart_id.clone();
    let mut chart = chart;
    chart.meta.id = (!id.is_empty()).then_some(id);
    Ok(guard.resettle(chart)?.clone())
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

// ---- notes ----
//
// One path, over the store. There used to be two: these commands wrote into
// the open chart's note list, and `write_journal_note` wrote into the journal
// folder. They validated identically, in three copies of the same sentence,
// and had drifted on whether an untagged note gets filed by the router. See
// `notes` for which copy survived.

/// Write a note against the chart a reading is open on — new when `id` is
/// absent, an amendment when it is present.
#[tauri::command]
fn write_note(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Option<String>,
    text: String,
    tags: Vec<String>,
) -> Result<Vec<Note>, String> {
    let chart_id = open_chart_id(&state)?;
    let locale = astro::i18n::Locale::parse(&state.session().chart()?.meta.locale);
    let store = store_of(&app)?;
    notes::write(
        &store,
        notes::Written { id, chart: &chart_id, text: &text, tags, locale, now_ms: now_ms() as i64 },
    )?;
    Ok(notes::under(&store, &chart_id).iter().map(StoredNote::note).collect())
}

/// Write a note against a chart named by id — the home page's path, where
/// there is no open reading and the sky is one the astrologer was only
/// looking at.
///
/// The chart record is minted if this is the first note about that moment, so
/// ten notes over one transit share one chart and "everything I wrote about
/// this sky" is a question with an answer.
#[tauri::command]
fn write_moment_note(
    app: AppHandle,
    id: Option<String>,
    text: String,
    tags: Vec<String>,
    anchor: Chart,
) -> Result<StoredNote, String> {
    let store = store_of(&app)?;
    // The place as the gazetteer says it, not as the webview typed it — the
    // id is the truth, and a stored label that disagreed with it would name
    // somewhere the note was not written over.
    let computed = chart_of(&anchor)?;
    let locale = astro::i18n::Locale::parse(&anchor.locale);
    let chart_id = match existing_chart(&store, &anchor) {
        Some(found) => found,
        None => {
            let record = Chart {
                id: store.mint_id::<Chart>(now_ms()),
                place: computed.meta.place.clone(),
                ..anchor
            };
            store.put(&record)?;
            record.id
        }
    };
    notes::write(
        &store,
        notes::Written { id, chart: &chart_id, text: &text, tags, locale, now_ms: now_ms() as i64 },
    )
}

/// The chart record for a sky already written about, matched on the seed and
/// the calculation codes — everything that decides the geometry. Two notes
/// over the same moment must land on one chart or neither can find the other.
fn existing_chart(store: &Store, want: &Chart) -> Option<String> {
    store
        .all::<Chart>()
        .into_iter()
        .find(|c| {
            c.seed == want.seed
                && c.house_system == want.house_system
                && c.ayanamsa == want.ayanamsa
                && c.kind == want.kind
                && c.person == want.person
        })
        .map(|c| c.id)
}

/// Merge a note into the one before it in reading order.
#[tauri::command]
fn merge_up(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<Note>, String> {
    let chart_id = open_chart_id(&state)?;
    let store = store_of(&app)?;
    notes::merge_up(&store, &chart_id, &id)?;
    Ok(notes::under(&store, &chart_id).iter().map(StoredNote::note).collect())
}

/// Remove a note. The frontend confirms first; removal is final.
#[tauri::command]
fn delete_note(app: AppHandle, id: String) -> Result<(), String> {
    store_of(&app)?.remove::<StoredNote>(&id)
}

/// Every note filed under the open reading's chart, in reading order.
#[tauri::command]
fn list_notes(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let chart_id = open_chart_id(&state)?;
    Ok(notes::under(&store_of(&app)?, &chart_id).iter().map(StoredNote::note).collect())
}

/// Recompute the chart a record names.
///
/// The webview sends the seed it was looking at, never the chart. Recomputing
/// here is what lets the backend render and label a note's sky rather than
/// taking the webview's word for it.
fn chart_of(record: &Chart) -> Result<ChartData, String> {
    let place = geo::by_id(record.seed.place_id).ok_or("that place is no longer in the gazetteer")?;
    let date = record
        .seed
        .date
        .parse()
        .map_err(|_| "a date as YYYY-MM-DD, e.g. 1990-07-13".to_string())?;
    let birth = resolve_input(
        "",
        date,
        &record.seed.time,
        place,
        astro::i18n::Locale::parse(&record.locale),
        systems::Codes::new(
            Some(record.house_system.as_str()),
            Some(if record.ayanamsa.is_some() { "sidereal" } else { "tropical" }),
            record.ayanamsa.as_deref(),
        ),
        systems::Codes::default(),
    )?;
    astro::chart::compute_chart(&birth)
}

/// One row of the journal view: every note the app holds, whatever sky it was
/// written over.
///
/// There used to be a `RowSource` enum here, splitting rows into the
/// journal's own notes and notes found inside saved readings, because those
/// were two different stores with two different ways of pointing back at a
/// note. One store, one kind of row: a note names a chart, and a chart may
/// name a person.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
struct JournalRow {
    /// The note's own id — what amends or deletes it.
    id: String,
    /// When it was written, RFC 3339.
    written_at: String,
    text: String,
    tags: Vec<String>,
    /// `tags` as words, in the row's own language — parallel to it.
    ///
    /// Composed here because a row carries no chart to derive them from, and
    /// because nobody searches for `planet:saturn`. The row's language is the
    /// chart's, not the app's: a note written under a Russian reading reads
    /// and searches in Russian wherever it is shown.
    tag_labels: Vec<String>,
    /// The sky it was written over — enough to restore it.
    chart: Chart,
    /// Whose sky it is, when it is anyone's.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    person: Option<Person>,
}

/// Ask a question of the notes. The journal view's own listing is this with
/// an empty query.
#[tauri::command]
fn query_notes(app: AppHandle, query: query::Query) -> Vec<JournalRow> {
    let Ok(store) = store_of(&app) else { return Vec::new() };
    rows_for(&store, query::run(&store, &query))
}

/// Save a question by name — new when `id` is absent, an amendment when it is
/// present.
#[tauri::command]
fn write_view(
    app: AppHandle,
    id: Option<String>,
    name: String,
    query: query::Query,
) -> Result<query::View, String> {
    let store = store_of(&app)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("a view needs a name".to_string());
    }
    let view = query::View {
        id: id.unwrap_or_else(|| store.mint_id::<query::View>(now_ms())),
        name: name.to_string(),
        query,
    };
    store.put(&view)?;
    Ok(view)
}

/// Every saved view, in the order they were made.
#[tauri::command]
fn list_views(app: AppHandle) -> Vec<query::View> {
    store_of(&app).map(|s| s.all::<query::View>()).unwrap_or_default()
}

#[tauri::command]
fn delete_view(app: AppHandle, id: String) -> Result<(), String> {
    store_of(&app)?.remove::<query::View>(&id)
}

/// Everyone the store knows about — the people filter's options.
#[tauri::command]
fn list_people(app: AppHandle) -> Vec<Person> {
    store_of(&app).map(|s| query::people(&s)).unwrap_or_default()
}

/// A tag id as the words a reader would search for.
///
/// A journal row carries no computed chart — that is what makes listing a few
/// hundred of them affordable — so the names are composed from the id and the
/// row's language alone. `planet:`, `sign:`, `house:` and `kind:` all name
/// themselves from `i18n`. An aspect's proper name would need its kind, which
/// the pair alone does not say, so it becomes its two endpoints: still the
/// words someone would type, which is what this is for.
fn tag_label(tag: &str, loc: astro::i18n::Locale) -> String {
    let Some((cat, slug)) = tag.split_once(':') else {
        return tag.to_string();
    };
    let named = match cat {
        "planet" => loc.planet_name(slug).to_string(),
        "sign" => loc.sign_name(slug).to_string(),
        "house" => slug.parse().map(|n| loc.house_name(n).to_string()).unwrap_or_default(),
        "kind" => loc.aspect_word(slug).to_string(),
        "aspect" => match slug.split_once('-') {
            Some((a, b)) => format!("{} – {}", loc.planet_name(a), loc.planet_name(b)),
            None => String::new(),
        },
        _ => String::new(),
    };
    // An id outside the vocabulary names nothing. Fall back to the id rather
    // than to a blank chip nobody could search for or click.
    if named.trim().is_empty() { tag.to_string() } else { named }
}

/// Every note the app holds, newest first.
///
/// This used to be two listings interleaved: the journal's own folder, plus
/// every note inside every saved `chart.json`. The second half opened and
/// fully parsed every reading in the library — "the cost is real and worth
/// naming", said the comment that used to be here, because a few transcribed
/// readings outnumber the typed notes by two orders of magnitude. Notes are
/// their own records now, so listing them reads notes and nothing else.
#[tauri::command]
fn list_journal(app: AppHandle) -> Vec<JournalRow> {
    let Ok(store) = store_of(&app) else { return Vec::new() };
    rows_for(&store, store.notes_newest_first())
}

/// Notes as rows: each one with the sky it was written over and whose it is.
///
/// Shared by the plain listing and by `query_notes`, so a searched row and a
/// listed row are the same thing — the search narrows what is shown, it does
/// not produce a different kind of result.
fn rows_for(store: &Store, notes: Vec<StoredNote>) -> Vec<JournalRow> {
    let charts: std::collections::HashMap<String, Chart> =
        store.all::<Chart>().into_iter().map(|c| (c.id.clone(), c)).collect();
    let people: std::collections::HashMap<String, Person> =
        store.all::<Person>().into_iter().map(|p| (p.id.clone(), p)).collect();
    notes
        .into_iter()
        .filter_map(|n| {
            // A note whose chart is gone has nothing to be read against, so it
            // is skipped rather than shown adrift — the same stance the store
            // takes on a file that will not parse.
            let chart = charts.get(&n.chart)?.clone();
            let loc = astro::i18n::Locale::parse(&chart.locale);
            Some(JournalRow {
                id: n.id,
                written_at: n.written_at,
                tag_labels: n.tags.iter().map(|t| tag_label(t, loc)).collect(),
                text: n.text,
                tags: n.tags,
                person: chart.person.as_ref().and_then(|id| people.get(id)).cloned(),
                chart,
            })
        })
        .collect()
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

//// Reopen a chart from the store — the readings panel's way back in.
///
/// This used to be `load_chart`, which took a filesystem path to a
/// `chart.json` inside a reading folder, deserialized an untrusted file and
/// validated it before anything downstream could touch it. A chart is a
/// record now: it is named by id, its geometry is recomputed from its seed
/// rather than read back, and there is no file for a hand-edit to have
/// corrupted.
#[tauri::command]
fn open_chart(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<ChartData, String> {
    let store = store_of(&app)?;
    let record = store.get::<Chart>(&id)?.ok_or("no such chart")?;
    let mut chart = chart_of(&record)?;
    chart.meta.id = Some(record.id.clone());
    if let Some(person) = record.person.as_ref().and_then(|p| store.get::<Person>(p).ok().flatten())
    {
        chart.meta.name = person.name;
        chart.meta.born = person.born;
    }
    // Branding is the practitioner's current setting rather than whatever was
    // stamped when the reading was first built — it is about who is exporting
    // it, not about the sky.
    let p = prefs::load(&app);
    chart.meta.astrologer = p.astrologer.clone().filter(|s| !s.trim().is_empty());
    chart.meta.logo = p.logo.as_deref().and_then(|l| prefs::logo_data_uri(Path::new(l)));

    state.session().open(Reading::reopened(chart.clone(), record.id, 0))?;
    Ok(chart)
}

/// One row of the readings panel: a chart, who it is of, and how much has
/// been written about it.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
struct ChartRow {
    chart: Chart,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    person: Option<Person>,
    /// How many notes are filed under it.
    notes: usize,
}

/// Every chart in the store, most written-about first.
///
/// This used to walk the readings folder and read a `chart.json` out of every
/// subfolder. A chart is a record, so listing them is a folder of records.
#[tauri::command]
fn list_charts(app: AppHandle) -> Vec<ChartRow> {
    let Ok(store) = store_of(&app) else { return Vec::new() };
    let people: std::collections::HashMap<String, Person> =
        store.all::<Person>().into_iter().map(|p| (p.id.clone(), p)).collect();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for n in store.all::<StoredNote>() {
        *counts.entry(n.chart).or_default() += 1;
    }
    let mut rows: Vec<ChartRow> = store
        .all::<Chart>()
        .into_iter()
        .map(|c| ChartRow {
            notes: counts.get(&c.id).copied().unwrap_or(0),
            person: c.person.as_ref().and_then(|id| people.get(id)).cloned(),
            chart: c,
        })
        .collect();
    // Newest chart first — ids are base-36 of the millisecond they were
    // minted, so id order is minting order.
    rows.sort_by(|a, b| b.chart.id.cmp(&a.chart.id));
    rows
}

/// Remove a chart and every note filed under it.
///
/// Final, and it says what it is taking: deleting a sky deletes what was
/// written about that sky, because a note with no chart has nothing to be
/// read against. The person stays — they may have other charts, and a person
/// is not deleted by deleting one of their skies.
#[tauri::command]
fn delete_chart(app: AppHandle, id: String) -> Result<(), String> {
    let store = store_of(&app)?;
    for note in notes::under(&store, &id) {
        store.remove::<StoredNote>(&note.id)?;
    }
    store.remove::<Chart>(&id)
}

/// The generated export name, `{name}_{date}.html` — the save dialog's
/// default, matching the library folder convention.
#[tauri::command]
fn artifact_filename(state: State<'_, AppState>) -> Result<String, String> {
    let guard = state.session();
    let chart = guard.chart()?;
    Ok(naming::artifact_name(&naming::stem(
        &chart.meta.name,
        chrono::Local::now().date_naive(),
    )))
}

// async: rendering + disk write stay off the main thread. Both exporters clone
// the chart out of a scoped lock first — rendering under the lock would stall
// every other command for the length of a disk write.
#[tauri::command]
async fn save_artifact(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let (chart, chart_id) = {
        let guard = state.session();
        let reading = guard.reading()?;
        (reading.chart.clone(), reading.chart_id.clone())
    };
    let notes = exported_notes(&app, &chart_id);
    tauri::async_runtime::spawn_blocking(move || {
        astro::emit::write_artifact(&chart, &notes, path.as_ref()).map(|()| path)
    })
    .await
    .map_err(|e| format!("artifact task failed: {e}"))?
}

/// The notes an export carries: whatever is filed under the chart.
///
/// An export with no store behind it is an export of the chart alone, which
/// is what a reading held only in the session has always been.
fn exported_notes(app: &AppHandle, chart_id: &str) -> Vec<Note> {
    store_of(app)
        .map(|store| notes::under(&store, chart_id).iter().map(StoredNote::note).collect())
        .unwrap_or_default()
}

/// The PDF rendition; page size comes from preferences (A4
/// unless set to letter).
#[tauri::command]
async fn save_pdf(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<String, String> {
    let size = astro::pdf::PageSize::from_pref(prefs::load(&app).page_size.as_deref())?;
    let (chart, chart_id) = {
        let guard = state.session();
        let reading = guard.reading()?;
        (reading.chart.clone(), reading.chart_id.clone())
    };
    let notes = exported_notes(&app, &chart_id);
    tauri::async_runtime::spawn_blocking(move || {
        astro::pdf::write_pdf(&chart, &notes, size, path.as_ref()).map(|()| path)
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
            write_note,
            write_moment_note,
            delete_note,
            list_notes,
            list_journal,
            query_notes,
            write_view,
            list_views,
            delete_view,
            list_people,
            get_preferences,
            set_preferences,
            open_licenses,
            list_models,
            artifact_filename,
            open_chart,
            list_charts,
            delete_chart
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

// The v1 migration stood here. It swept the readings library stamping an id
// onto every saved chart, so that a note could be about a reading rather than
// about a folder path that moves. Both halves of that are gone: charts are
// records with ids of their own, and the folders it swept are not where
// anything lives any more. It had nothing left to migrate to.

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

    fn chart_fixture() -> ChartData {
        astro::fixtures::berlin_chart()
    }

    // Writing, amending, merging, deleting and re-tagging a note are all one
    // path now, over the store, and their tests live with it in `notes`.
    // What is left here is the command layer's own: recomputing a chart from
    // a record, resolving a birth input, and naming a tag.

    fn ex(id: &str, text: &str, tags: &[&str]) -> Note {
        Note {
            id: id.into(),
            at: None,
            text: text.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn chart_json_round_trips_for_loading() {
        // The load_chart path relies on ChartData deserializing from the same
        let chart = chart_fixture();
        let json = serde_json::to_string_pretty(&chart).unwrap();
        let back: ChartData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.meta.name, chart.meta.name);
        assert_eq!(back.planets.len(), chart.planets.len());
        assert_eq!(back.aspects.len(), chart.aspects.len());
        // `Aspect::kind` is #[serde(skip)] — it defaults to "" on load.
        assert!(back.aspects.iter().all(|a| a.kind.is_empty()));
    }

    /// A reproject changes only the geometry. The practitioner's branding and
    /// the birth seed that makes the *next* recalculation possible must cross
    /// onto the recomputed chart.
    ///
    /// The notes used to cross here too, and the test said so. They no longer
    /// need to — they name the chart's id, and the id survives a reproject —
    /// so what is left to check is that their tags would still be valid,
    /// which is what makes a house-system swap safe for them.
    #[test]
    fn a_reproject_carries_branding_and_the_seed_onto_the_new_geometry() {
        let mut old = chart_fixture();
        old.meta.birth = Some(astro::contract::BirthSeed {
            place_id: 2950159,
            date: "1990-07-13".into(),
            time: "14:30".into(),
        });
        old.meta.astrologer = Some("A. Practitioner".into());
        old.meta.logo = Some("data:image/png;base64,AAAA".into());

        let carried = Carried::from(&old).expect("a seeded chart can be recalculated");
        assert_eq!(carried.name, old.meta.name);
        assert_eq!(carried.locale, astro::i18n::Locale::En);

        // The recomputed chart arrives with fresh geometry and nothing else.
        let mut fresh = {
            let mut input = astro::fixtures::berlin();
            input.house_system = systems::house_system("placidus").unwrap();
            astro::chart::compute_chart(&input).unwrap()
        };
        assert_ne!(fresh.house_cusps, old.house_cusps, "the geometry really changed");

        carried.onto(&mut fresh);

        assert_eq!(fresh.meta.astrologer.as_deref(), Some("A. Practitioner"));
        assert_eq!(fresh.meta.logo.as_deref(), Some("data:image/png;base64,AAAA"));
        assert!(fresh.meta.birth.is_some(), "still recalculable afterwards");
        assert!(fresh.validate().is_ok(), "{:?}", fresh.validate());
        // A note filed against the old geometry stays filed under tags the new
        // one admits, which is what makes the swap safe for it.
        let filed = ex("x1", "The sun in cancer.", &["planet:sun", "sign:cancer"]);
        assert!(filed.validate().is_ok());
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





    // ---- the journal's cross-reading view ----

    fn anchor() -> Chart {
        Chart {
            id: "c1".into(),
            kind: "moment".into(),
            seed: astro::contract::BirthSeed {
                place_id: 2_950_159,
                date: "2026-08-04".into(),
                time: "09:12".into(),
            },
            place: "Berlin, Germany".into(),
            house_system: "whole-sign".into(),
            ayanamsa: None,
            locale: "en".into(),
            person: None,
        }
    }

    /// The webview sends a seed, never a chart. A note's tags no longer need
    /// the recompute — the vocabulary is global — but the sky does: the place
    /// label stored on the anchor is the gazetteer's, not the webview's, and
    /// the wheel a note is read against has to come from somewhere.
    #[test]
    fn an_anchor_recomputes_the_chart_it_names() {
        let chart = chart_of(&anchor()).expect("a real place and a real date compute");
        assert!(chart.planet("planet:sun").is_some());
        assert_eq!(chart.planets.len(), 11);

        let mut bad = anchor();
        bad.seed.date = "the fourth".into();
        assert!(chart_of(&bad).is_err(), "an unparseable date is refused, not defaulted");

        let mut bad = anchor();
        bad.seed.place_id = 0;
        assert!(chart_of(&bad).is_err(), "a place that is not in the gazetteer is refused");
    }

    /// Sidereal is carried by the presence of an ayanamsa rather than by a
    /// separate zodiac field, so a note written under one recomputes under it.
    #[test]
    fn an_anchors_calculation_survives_the_round_trip() {
        let tropical = chart_of(&anchor()).unwrap();
        let mut sidereal_anchor = anchor();
        sidereal_anchor.ayanamsa = Some("lahiri".into());
        let sidereal = chart_of(&sidereal_anchor).unwrap();
        assert_ne!(
            tropical.planets[0].lon, sidereal.planets[0].lon,
            "the ayanamsa reached the computation"
        );
        assert_eq!(sidereal.meta.ayanamsa.as_deref(), Some("lahiri"));

        let mut placidus = anchor();
        placidus.house_system = "placidus".into();
        assert_eq!(chart_of(&placidus).unwrap().meta.house_system, "placidus");

        let mut nonsense = anchor();
        nonsense.house_system = "not-a-system".into();
        assert!(chart_of(&nonsense).is_err(), "an unknown code is refused, not defaulted");
    }


    /// A journal row carries no chart, so its elements have to name themselves
    /// from the id and the row's language alone — and nobody searches for
    /// `planet:saturn`.
    #[test]
    fn a_tag_names_itself_in_the_rows_own_language() {
        use astro::i18n::Locale::{En, Ru};
        assert_eq!(tag_label("planet:saturn", En), "Saturn");
        assert_eq!(tag_label("sign:leo", En), "Leo");
        assert_eq!(tag_label("house:5", En), "Fifth House");
        assert_eq!(tag_label("aspect:sun-moon", En), "Sun – Moon");
        assert_eq!(tag_label("kind:trine", En), "trine");

        // The reading's language, not the app's — a note written under a
        // Russian reading searches in Russian wherever it is shown.
        assert_ne!(tag_label("planet:saturn", Ru), tag_label("planet:saturn", En));
        assert!(!tag_label("planet:saturn", Ru).is_empty());
        assert!(!tag_label("house:5", Ru).is_empty());
    }

    /// An id nothing can name still has to be searchable and clickable, so it
    /// falls back to itself rather than to a blank chip.
    #[test]
    fn an_unnameable_tag_falls_back_to_its_id() {
        use astro::i18n::Locale::En;
        for odd in ["planet:vulcan", "house:99", "house:not-a-number", "aspect:lonely", "nonsense"] {
            assert_eq!(tag_label(odd, En), odd, "{odd} named itself something else");
        }
    }

}
