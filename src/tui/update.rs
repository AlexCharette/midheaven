//! The Msg/Cmd vocabulary and the pure `update` function of the Elm loop.
//! Key events are decoded to semantic messages by [`decode`]; side effects
//! leave `update` only as [`Cmd`]s for the runtime to execute.

use super::model::{Field, Form, Job, Model, Reading, Screen};
use astro::TranscriptSource;
use astro::chart::BirthInput;
use astro::contract::{ChartData, Mode};
use astro::geo;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};

pub enum Msg {
    // form
    Input(char),
    Backspace,
    NextField,
    PrevField,
    SuggestionDown,
    SuggestionUp,
    /// Enter: pick the highlighted suggestion, or submit from the last field.
    Accept,
    Dismiss,
    Submit,
    // reading
    CursorLeft,
    CursorRight,
    CursorUp,
    CursorDown,
    ToggleSel,
    ClearSel,
    ToggleMode,
    ScrollDown,
    ScrollUp,
    Emit,
    Back,
    // global + effect results
    Quit,
    /// Whole-percent transcription progress from the background build.
    Progress(i32),
    Built(Result<Box<ChartData>, String>),
    Emitted(Result<String, String>),
}


pub enum Cmd {
    /// Run the pipeline on a background thread: transcribe when the source
    /// is audio (emitting [`Msg::Progress`]), compute, route + verify.
    /// Resolves to [`Msg::Built`].
    Build {
        input: BirthInput,
        source: TranscriptSource,
    },
    /// Write an already-rendered artifact. Self-contained: the interpreter
    /// needs no view of the Model. Resolves to [`Msg::Emitted`].
    WriteFile { path: String, contents: String },
}

/// Key event → semantic message, per screen. Returns None for keys that mean
/// nothing in the current state.
pub fn decode(model: &Model, key: KeyEvent) -> Option<Msg> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Msg::Quit);
    }
    match &model.screen {
        Screen::Form(form) => match key.code {
            KeyCode::Char(c) => Some(Msg::Input(c)),
            KeyCode::Backspace => Some(Msg::Backspace),
            KeyCode::Tab => Some(Msg::NextField),
            KeyCode::BackTab => Some(Msg::PrevField),
            KeyCode::Down if form.sel.is_some() => Some(Msg::SuggestionDown),
            KeyCode::Up if form.sel.is_some() => Some(Msg::SuggestionUp),
            KeyCode::Down => Some(Msg::NextField),
            KeyCode::Up => Some(Msg::PrevField),
            KeyCode::Enter => Some(Msg::Accept),
            KeyCode::Esc => Some(Msg::Dismiss),
            KeyCode::F(5) => Some(Msg::Submit),
            _ => None,
        },
        Screen::Reading(_) => match key.code {
            KeyCode::Char('q') => Some(Msg::Quit),
            KeyCode::Char('b') => Some(Msg::Back),
            KeyCode::Char('e') => Some(Msg::Emit),
            KeyCode::Char('a') => Some(Msg::ToggleMode),
            KeyCode::Char('c') => Some(Msg::ClearSel),
            KeyCode::Char(' ') => Some(Msg::ToggleSel),
            KeyCode::Enter => Some(Msg::ToggleSel),
            KeyCode::Left | KeyCode::Char('h') => Some(Msg::CursorLeft),
            KeyCode::Right | KeyCode::Char('l') => Some(Msg::CursorRight),
            KeyCode::Up => Some(Msg::CursorUp),
            KeyCode::Down => Some(Msg::CursorDown),
            KeyCode::Char('k') => Some(Msg::ScrollUp),
            KeyCode::Char('j') => Some(Msg::ScrollDown),
            _ => None,
        },
    }
}

pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Quit => {
            model.should_quit = true;
            Vec::new()
        }
        Msg::Progress(pct) => {
            if let Some(Job::Transcribing(p)) = &mut model.job {
                *p = pct;
            }
            Vec::new()
        }
        Msg::Built(Ok(chart)) => {
            model.job = None;
            // Build commands are only issued by the form's submit.
            let Screen::Form(form) = &model.screen else {
                return Vec::new();
            };
            let out = form.out.clone();
            let n = chart.excerpts.len();
            model.screen = Screen::Reading(Reading::new(chart, out));
            model.status = format!("{n} passages routed past the verify gate");
            Vec::new()
        }
        Msg::Built(Err(e)) => {
            model.job = None;
            model.status = format!("✗ {e}");
            Vec::new()
        }
        Msg::Emitted(Ok(path)) => {
            model.status = format!("wrote {path} ☞ open it in a browser");
            Vec::new()
        }
        Msg::Emitted(Err(e)) => {
            model.status = format!("✗ {e}");
            Vec::new()
        }
        Msg::Back => {
            if matches!(model.screen, Screen::Reading(_)) {
                model.screen = Screen::Form(Form::default());
                model.status = String::new();
            }
            Vec::new()
        }
        other => match &mut model.screen {
            Screen::Form(form) => {
                let cmds = update_form(form, other);
                // Busy-gating keys off the actual command, not off which
                // message might have submitted: navigation stays free while
                // a build runs; only a second Build is refused.
                if let Some(Cmd::Build { source, .. }) = cmds.first() {
                    if model.job.is_some() {
                        model.status = "still working — the figure is on its way".to_string();
                        return Vec::new();
                    }
                    model.job = Some(match source {
                        TranscriptSource::Audio { .. } => Job::Transcribing(0),
                        _ => Job::Computing,
                    });
                    model.status = String::new();
                }
                cmds
            }
            Screen::Reading(reading) => update_reading(reading, &mut model.status, other),
        },
    }
}

/// One keystroke of editing the focused field, with its shared bookkeeping:
/// errors clear, a Place edit invalidates the picked place, and only a Place
/// edit re-queries the gazetteer (`geo::search` is a pure read of process-
/// static data, so it may live in update).
fn edit_field(form: &mut Form, edit: impl FnOnce(&mut String)) {
    form.error = None;
    if form.focus == Field::Place {
        form.picked = None;
    }
    edit(form.value_mut(form.focus));
    if form.focus == Field::Place && !form.place_query.trim().is_empty() {
        form.suggestions = geo::search(&form.place_query, 6);
        form.sel = if form.suggestions.is_empty() { None } else { Some(0) };
    } else {
        close_suggestions(form);
    }
}

fn close_suggestions(form: &mut Form) {
    form.suggestions.clear();
    form.sel = None;
}

fn update_form(form: &mut Form, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Input(c) => edit_field(form, |v| v.push(c)),
        Msg::Backspace => edit_field(form, |v| {
            v.pop();
        }),
        Msg::NextField => {
            form.focus = form.focus.next();
            close_suggestions(form);
        }
        Msg::PrevField => {
            form.focus = form.focus.prev();
            close_suggestions(form);
        }
        Msg::SuggestionDown => {
            if let Some(i) = form.sel {
                form.sel = Some((i + 1).min(form.suggestions.len().saturating_sub(1)));
            }
        }
        Msg::SuggestionUp => {
            if let Some(i) = form.sel {
                form.sel = Some(i.saturating_sub(1));
            }
        }
        Msg::Dismiss => close_suggestions(form),
        Msg::Accept => {
            if let Some(i) = form.sel {
                if let Some(p) = form.suggestions.get(i).copied() {
                    form.picked = Some(p);
                    form.place_query = p.label();
                    close_suggestions(form);
                    form.focus = form.focus.next();
                }
            } else if form.focus == Field::Out {
                return submit(form);
            } else {
                form.focus = form.focus.next();
            }
        }
        Msg::Submit => return submit(form),
        _ => {}
    }
    Vec::new()
}

/// Validate the form; on success hand the runtime a Build command.
fn submit(form: &mut Form) -> Vec<Cmd> {
    let fail = |form: &mut Form, field: Field, msg: &str| {
        form.error = Some((field, msg.to_string()));
        form.focus = field;
        Vec::new()
    };
    let Ok(date) = form.date.parse::<chrono::NaiveDate>() else {
        return fail(form, Field::Date, "a date as YYYY-MM-DD, e.g. 1990-07-13");
    };
    let Ok(time) = astro::chart::parse_time(&form.time) else {
        return fail(form, Field::Time, "a time as HH:MM, e.g. 14:30");
    };
    let place = match form.picked {
        Some(p) => p,
        None => match geo::resolve(&form.place_query) {
            geo::Resolution::Match(p) => {
                form.picked = Some(p);
                p
            }
            geo::Resolution::Ambiguous(_) => {
                return fail(form, Field::Place, "several places match — pick one from the list");
            }
            geo::Resolution::NotFound => {
                return fail(form, Field::Place, "no such place in the gazetteer");
            }
        },
    };
    let transcript = form.transcript.trim();
    if !transcript.is_empty() && !Path::new(transcript).exists() {
        return fail(form, Field::Transcript, "no file at this path");
    }
    // decided by content (RIFF magic), not by file name
    let is_audio = !transcript.is_empty() && astro::transcribe::is_audio(Path::new(transcript));
    let model_path = form.model.trim();
    if is_audio {
        if model_path.is_empty() {
            return fail(form, Field::Model, "an audio transcript needs a ggml whisper model");
        }
        if !Path::new(model_path).exists() {
            return fail(form, Field::Model, "no model file at this path");
        }
    }
    if form.out.trim().is_empty() {
        return fail(form, Field::Out, "the artifact needs a path");
    }
    let source = if transcript.is_empty() {
        TranscriptSource::None
    } else if is_audio {
        TranscriptSource::Audio {
            wav: PathBuf::from(transcript),
            model: PathBuf::from(model_path),
        }
    } else {
        TranscriptSource::File(PathBuf::from(transcript))
    };
    let name = if form.name.trim().is_empty() { "Anonymous" } else { form.name.trim() };
    vec![Cmd::Build {
        input: BirthInput {
            name: name.to_string(),
            date,
            time,
            lat: place.lat,
            lon: place.lon,
            tz: place.tz,
            place: place.label(),
        },
        source,
    }]
}

fn update_reading(reading: &mut Reading, status: &mut String, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::CursorLeft => {
            let n = reading.columns.len();
            reading.cursor.0 = (reading.cursor.0 + n - 1) % n;
            clamp_row(reading);
        }
        Msg::CursorRight => {
            reading.cursor.0 = (reading.cursor.0 + 1) % reading.columns.len();
            clamp_row(reading);
        }
        Msg::CursorUp => reading.cursor.1 = reading.cursor.1.saturating_sub(1),
        Msg::CursorDown => {
            let max = reading.columns[reading.cursor.0].entries.len().saturating_sub(1);
            reading.cursor.1 = (reading.cursor.1 + 1).min(max);
        }
        Msg::ToggleSel => {
            if let Some(entry) = reading.cursor_entry() {
                let tag = entry.tag.clone();
                if !reading.selected.remove(&tag) {
                    reading.selected.insert(tag);
                }
                reading.scroll = 0;
            }
        }
        Msg::ClearSel => {
            reading.selected.clear();
            reading.scroll = 0;
        }
        Msg::ToggleMode => {
            reading.mode = match reading.mode {
                Mode::Any => Mode::All,
                Mode::All => Mode::Any,
            };
            reading.scroll = 0;
        }
        Msg::ScrollDown => reading.scroll = reading.scroll.saturating_add(2),
        Msg::ScrollUp => reading.scroll = reading.scroll.saturating_sub(2),
        Msg::Emit => {
            // Rendering is pure, so it happens here; only the write is an
            // effect. No success status: Emitted lands before the next frame.
            return match astro::emit::emit(&reading.chart) {
                Ok(contents) => vec![Cmd::WriteFile { path: reading.out.clone(), contents }],
                Err(e) => {
                    *status = format!("✗ {e}");
                    Vec::new()
                }
            };
        }
        _ => {}
    }
    Vec::new()
}

fn clamp_row(reading: &mut Reading) {
    let max = reading.columns[reading.cursor.0].entries.len().saturating_sub(1);
    reading.cursor.1 = reading.cursor.1.min(max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn type_str(model: &mut Model, s: &str) {
        for c in s.chars() {
            update(model, Msg::Input(c));
        }
    }

    fn form(model: &Model) -> &Form {
        match &model.screen {
            Screen::Form(f) => f,
            _ => panic!("expected form"),
        }
    }

    /// A Model whose form holds valid Berlin birth data.
    fn filled_form() -> Model {
        let mut m = Model::default();
        if let Screen::Form(f) = &mut m.screen {
            f.date = "1990-07-13".into();
            f.time = "14:30".into();
            f.place_query = "berlin".into();
        }
        m
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, b"x").unwrap();
        path
    }

    #[test]
    fn typing_a_place_offers_suggestions_and_enter_picks() {
        let mut m = Model::default();
        // move focus to the place field
        for _ in 0..3 {
            update(&mut m, Msg::NextField);
        }
        assert_eq!(form(&m).focus, Field::Place);
        type_str(&mut m, "berlin");
        assert!(!form(&m).suggestions.is_empty(), "expected suggestions for 'berlin'");
        assert_eq!(form(&m).sel, Some(0));
        update(&mut m, Msg::Accept);
        let f = form(&m);
        let p = f.picked.expect("place should be picked");
        assert_eq!(p.cc, "DE");
        assert!((p.lat - 52.52).abs() < 0.1);
        assert!(f.suggestions.is_empty());
    }

    #[test]
    fn invalid_date_is_a_field_error_not_a_command() {
        let mut m = filled_form();
        if let Screen::Form(f) = &mut m.screen {
            f.date = "13/07/1990".into();
        }
        let cmds = update(&mut m, Msg::Submit);
        assert!(cmds.is_empty());
        let f = form(&m);
        assert!(matches!(f.error, Some((Field::Date, _))));
        assert_eq!(f.focus, Field::Date);
    }

    #[test]
    fn valid_form_yields_build_command() {
        let mut m = filled_form();
        let cmds = update(&mut m, Msg::Submit);
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            Cmd::Build { input, source } => {
                assert!((input.lat - 52.52).abs() < 0.1);
                assert_eq!(input.tz, chrono_tz::Europe::Berlin);
                assert!(matches!(source, TranscriptSource::None));
            }
            _ => panic!("expected Build"),
        }
        assert_eq!(form(&m).out, "reading.html");
        assert_eq!(m.job, Some(Job::Computing), "a submitted build marks a job in flight");
    }

    use crate::tui::testkit::reading_model;

    fn reading(model: &Model) -> &Reading {
        match &model.screen {
            Screen::Reading(r) => r,
            _ => panic!("expected reading"),
        }
    }

    #[test]
    fn tag_toggle_filters_passages_any_vs_all() {
        let mut m = reading_model();
        let total = reading(&m).chart.excerpts.len();
        assert_eq!(reading(&m).visible().len(), total);
        // planets column, row 0 = Sun
        update(&mut m, Msg::ToggleSel);
        let sun_any = reading(&m).visible().len();
        assert!(sun_any < total && sun_any > 0, "sun filter should narrow: {sun_any}/{total}");
        // add house:10 (houses column is index 2, row 9)
        if let Screen::Reading(r) = &mut m.screen {
            r.cursor = (2, 9);
        }
        update(&mut m, Msg::ToggleSel);
        let any = reading(&m).visible().len();
        update(&mut m, Msg::ToggleMode);
        let all = reading(&m).visible().len();
        assert!(all <= any, "All ({all}) must be at most Any ({any})");
        // the sample transcript has exactly one sun-in-tenth passage
        assert_eq!(all, 1);
    }

    /// A minimal RIFF/WAVE file so content sniffing recognizes audio.
    fn temp_wav(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        hound::WavWriter::create(&path, spec).unwrap().finalize().unwrap();
        path
    }

    #[test]
    fn wav_transcript_requires_a_model() {
        let mut m = filled_form();
        let wav = temp_wav("astro-test-empty.wav");
        if let Screen::Form(f) = &mut m.screen {
            f.transcript = wav.to_string_lossy().into_owned();
        }
        let cmds = update(&mut m, Msg::Submit);
        assert!(cmds.is_empty());
        assert!(matches!(form(&m).error, Some((Field::Model, _))));
        assert!(m.job.is_none());
    }

    #[test]
    fn wav_with_model_builds_audio_source_and_marks_busy() {
        let mut m = filled_form();
        let wav = temp_wav("astro-test-a.wav");
        let ggml = temp_file("astro-test-model.bin");
        if let Screen::Form(f) = &mut m.screen {
            f.transcript = wav.to_string_lossy().into_owned();
            f.model = ggml.to_string_lossy().into_owned();
        }
        let cmds = update(&mut m, Msg::Submit);
        assert!(matches!(
            cmds.as_slice(),
            [Cmd::Build { source: TranscriptSource::Audio { .. }, .. }]
        ));
        assert_eq!(m.job, Some(Job::Transcribing(0)));
        // a second submit while a job runs is refused
        assert!(update(&mut m, Msg::Submit).is_empty());
        assert!(m.status.contains("still working"));
        // progress lands in the job, not in prose; a failed build clears it
        update(&mut m, Msg::Progress(42));
        assert_eq!(m.job, Some(Job::Transcribing(42)));
        update(&mut m, Msg::Built(Err("boom".into())));
        assert!(m.job.is_none());
    }

    #[test]
    fn emit_only_from_reading() {
        let mut m = Model::default();
        assert!(update(&mut m, Msg::Emit).is_empty());
        let mut r = reading_model();
        let cmds = update(&mut r, Msg::Emit);
        assert!(matches!(cmds.as_slice(), [Cmd::WriteFile { .. }]));
    }

    #[test]
    fn ctrl_c_always_quits() {
        let m = Model::default();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(decode(&m, key), Some(Msg::Quit)));
    }
}
