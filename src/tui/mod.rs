//! The TEA runtime: draw → poll → decode → update → execute commands →
//! feed results back as messages. All rendering lives in `view`, all state
//! transitions in `update`; this module owns the terminal and the effects.

mod model;
mod theme;
mod update;
mod view;
mod wheel;


use model::Model;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;
use update::{Cmd, Msg, decode, update};

pub fn run() -> Result<(), String> {
    // The gazetteer's one-time parse happens before raw mode, so the first
    // keystroke in the place field is instant.
    eprintln!("consulting the gazetteer…");
    astro::geo::warm();

    let mut terminal = ratatui::init();
    let mut model = Model::default();
    let mut dirty = true; // redraw only when something changed
    let result = loop {
        if dirty {
            if let Err(e) = terminal.draw(|f| view::view(&model, f)) {
                break Err(e.to_string());
            }
            dirty = false;
        }
        match event::poll(Duration::from_millis(250)) {
            Ok(false) => {}
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if let Some(msg) = decode(&model, key) {
                        dispatch(&mut model, msg);
                        dirty = true;
                    }
                }
                Ok(_) => dirty = true, // resize etc.
                Err(e) => break Err(e.to_string()),
            },
            Err(e) => break Err(e.to_string()),
        }
        if model.should_quit {
            break Ok(());
        }
    };
    ratatui::restore();
    result
}

/// Run a message through update, then execute the commands it produced,
/// feeding each command's result message back through update.
fn dispatch(model: &mut Model, msg: Msg) {
    let mut msgs = vec![msg];
    while let Some(msg) = msgs.pop() {
        for cmd in update(model, msg) {
            msgs.extend(execute(cmd));
        }
    }
}

/// The effect interpreter: commands are self-contained, so this needs no
/// view of the Model — the only place the TUI touches the filesystem.
fn execute(cmd: Cmd) -> Option<Msg> {
    match cmd {
        Cmd::Build { input, transcript } => Some(Msg::Built(
            astro::build_reading(&input, transcript.as_deref())
                .map(|(chart, _)| Box::new(chart)),
        )),
        Cmd::WriteFile { path, contents } => Some(Msg::Emitted(
            std::fs::write(&path, contents)
                .map_err(|e| format!("cannot write {path}: {e}"))
                .map(|()| path),
        )),
    }
}

/// Shared fixtures for the TEA unit tests and view snapshots.
#[cfg(test)]
pub(crate) mod testkit {
    use super::model::{Model, Reading, Screen};

    /// A Model on the Reading screen with the sample chart + transcript.
    pub(crate) fn reading_model() -> Model {
        let input = astro::chart::BirthInput {
            name: "Sample Chart".into(),
            date: "1990-07-13".parse().unwrap(),
            time: "14:30:00".parse().unwrap(),
            lat: 52.52,
            lon: 13.405,
            tz: chrono_tz::Europe::Berlin,
            place: "Berlin, Germany".into(),
        };
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/transcript.jsonl");
        let (chart, _) = astro::build_reading(&input, Some(path.as_ref())).unwrap();
        Model {
            screen: Screen::Reading(Reading::new(Box::new(chart), "reading.html".into())),
            ..Model::default()
        }
    }
}
