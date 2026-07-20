//! The Msg/Cmd vocabulary and the pure `update` function of the Elm loop.
//! Key events are decoded to semantic messages by [`decode`]; side effects
//! leave `update` only as [`Cmd`]s for the runtime to execute.

use super::model::{Field, Form, Mode, Model, Reading, Screen};
use astro::chart::BirthInput;
use astro::contract::ChartData;
use astro::geo;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

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
    Built(Result<Box<ChartData>, String>),
    Emitted(Result<String, String>),
}

pub enum Cmd {
    /// Run the pipeline: (optionally) load the transcript, compute the
    /// chart, route + verify. Resolves to [`Msg::Built`].
    Build {
        input: BirthInput,
        transcript: Option<PathBuf>,
    },
    /// Emit the current chart to its artifact path. Resolves to [`Msg::Emitted`].
    Emit,
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
        Msg::Built(Ok(chart)) => {
            let out = match &model.screen {
                Screen::Form(f) => f.out.clone(),
                Screen::Reading(r) => r.out.clone(),
            };
            let n = chart.excerpts.len();
            model.screen = Screen::Reading(Reading::new(chart, out));
            model.status = format!("{n} passages routed past the verify gate");
            Vec::new()
        }
        Msg::Built(Err(e)) => {
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
            Screen::Form(form) => update_form(form, &mut model.status, other),
            Screen::Reading(reading) => update_reading(reading, &mut model.status, other),
        },
    }
}

fn refresh_suggestions(form: &mut Form) {
    if form.focus == Field::Place && !form.place_query.trim().is_empty() {
        form.suggestions = geo::search(&form.place_query, 6);
        form.sel = if form.suggestions.is_empty() { None } else { Some(0) };
    } else {
        form.suggestions.clear();
        form.sel = None;
    }
}

fn update_form(form: &mut Form, status: &mut String, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Input(c) => {
            form.error = None;
            if form.focus == Field::Place {
                form.picked = None;
            }
            form.value_mut(form.focus).push(c);
            refresh_suggestions(form);
        }
        Msg::Backspace => {
            form.error = None;
            if form.focus == Field::Place {
                form.picked = None;
            }
            form.value_mut(form.focus).pop();
            refresh_suggestions(form);
        }
        Msg::NextField => {
            form.focus = form.focus.next();
            refresh_suggestions(form);
        }
        Msg::PrevField => {
            form.focus = form.focus.prev();
            refresh_suggestions(form);
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
        Msg::Dismiss => {
            form.suggestions.clear();
            form.sel = None;
        }
        Msg::Accept => {
            if let Some(i) = form.sel {
                if let Some(p) = form.suggestions.get(i).copied() {
                    form.picked = Some(p);
                    form.place_query = p.label();
                    form.suggestions.clear();
                    form.sel = None;
                    form.focus = form.focus.next();
                }
            } else if form.focus == Field::Out {
                return submit(form, status);
            } else {
                form.focus = form.focus.next();
            }
        }
        Msg::Submit => return submit(form, status),
        _ => {}
    }
    Vec::new()
}

/// Validate the form; on success hand the runtime a Build command.
fn submit(form: &mut Form, status: &mut String) -> Vec<Cmd> {
    let fail = |form: &mut Form, field: Field, msg: &str| {
        form.error = Some((field, msg.to_string()));
        form.focus = field;
        Vec::new()
    };
    let Ok(date) = form.date.parse::<chrono::NaiveDate>() else {
        return fail(form, Field::Date, "a date as YYYY-MM-DD, e.g. 1990-07-13");
    };
    let time_str = if form.time.len() == 5 { format!("{}:00", form.time) } else { form.time.clone() };
    let Ok(time) = time_str.parse::<chrono::NaiveTime>() else {
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
    if !transcript.is_empty() && !std::path::Path::new(transcript).exists() {
        return fail(form, Field::Transcript, "no file at this path");
    }
    if form.out.trim().is_empty() {
        return fail(form, Field::Out, "the artifact needs a path");
    }
    let name = if form.name.trim().is_empty() { "Anonymous" } else { form.name.trim() };
    *status = "computing the figure…".to_string();
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
        transcript: (!transcript.is_empty()).then(|| PathBuf::from(transcript)),
    }]
}

fn update_reading(reading: &mut Reading, status: &mut String, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::CursorLeft => {
            reading.cursor.0 = (reading.cursor.0 + 3) % 4;
            clamp_row(reading);
        }
        Msg::CursorRight => {
            reading.cursor.0 = (reading.cursor.0 + 1) % 4;
            clamp_row(reading);
        }
        Msg::CursorUp => reading.cursor.1 = reading.cursor.1.saturating_sub(1),
        Msg::CursorDown => {
            let max = reading.columns[reading.cursor.0].len().saturating_sub(1);
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
            *status = "engraving…".to_string();
            return vec![Cmd::Emit];
        }
        _ => {}
    }
    Vec::new()
}

fn clamp_row(reading: &mut Reading) {
    let max = reading.columns[reading.cursor.0].len().saturating_sub(1);
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
        let mut m = Model::default();
        if let Screen::Form(f) = &mut m.screen {
            f.date = "13/07/1990".into();
            f.time = "14:30".into();
            f.place_query = "berlin".into();
        }
        let cmds = update(&mut m, Msg::Submit);
        assert!(cmds.is_empty());
        let f = form(&m);
        assert!(matches!(f.error, Some((Field::Date, _))));
        assert_eq!(f.focus, Field::Date);
    }

    #[test]
    fn valid_form_yields_build_command() {
        let mut m = Model::default();
        if let Screen::Form(f) = &mut m.screen {
            f.name = "Test".into();
            f.date = "1990-07-13".into();
            f.time = "14:30".into();
            f.place_query = "berlin".into(); // resolves via dominance rule
        }
        let cmds = update(&mut m, Msg::Submit);
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            Cmd::Build { input, transcript } => {
                assert!((input.lat - 52.52).abs() < 0.1);
                assert_eq!(input.tz, chrono_tz::Europe::Berlin);
                assert!(transcript.is_none());
            }
            _ => panic!("expected Build"),
        }
        assert_eq!(form(&m).out, "reading.html");
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

    #[test]
    fn emit_only_from_reading() {
        let mut m = Model::default();
        assert!(update(&mut m, Msg::Emit).is_empty());
        let mut r = reading_model();
        let cmds = update(&mut r, Msg::Emit);
        assert!(matches!(cmds.as_slice(), [Cmd::Emit]));
    }

    #[test]
    fn ctrl_c_always_quits() {
        let m = Model::default();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(decode(&m, key), Some(Msg::Quit)));
    }
}
