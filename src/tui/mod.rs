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
use std::sync::mpsc;
use std::time::Duration;
use update::{Cmd, Msg, decode, update};

pub fn run() -> Result<(), String> {
    // The gazetteer's one-time parse happens before raw mode, so the first
    // keystroke in the place field is instant.
    eprintln!("consulting the gazetteer…");
    astro::geo::warm();

    let mut terminal = ratatui::init();
    let mut model = Model::default();
    // Effect results (and background-thread progress) flow back as messages.
    let (tx, rx) = mpsc::channel::<Msg>();
    let mut dirty = true; // redraw only when something changed
    let result = loop {
        while let Ok(msg) = rx.try_recv() {
            dispatch(&mut model, msg, &tx);
            dirty = true;
        }
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
                        dispatch(&mut model, msg, &tx);
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

/// Run a message through update, handing its commands to the interpreter.
/// Command results come back through the channel on a later loop turn.
fn dispatch(model: &mut Model, msg: Msg, tx: &mpsc::Sender<Msg>) {
    for cmd in update(model, msg) {
        execute(cmd, tx);
    }
}

/// The effect interpreter: commands are self-contained, so this needs no
/// view of the Model. Fast effects reply immediately; the pipeline build
/// runs on a background thread (transcription can take minutes) and streams
/// progress + result back through the channel.
fn execute(cmd: Cmd, tx: &mpsc::Sender<Msg>) {
    match cmd {
        Cmd::Build { input, source } => {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let progress_tx = tx.clone();
                let result = astro::build_reading(&input, source, move |pct| {
                    let _ = progress_tx.send(Msg::Progress(pct));
                });
                let _ = tx.send(Msg::Built(result.map(|(chart, _)| Box::new(chart))));
            });
        }
        Cmd::WriteFile { path, contents } => {
            let _ = tx.send(Msg::Emitted(
                std::fs::write(&path, contents)
                    .map_err(|e| format!("cannot write {path}: {e}"))
                    .map(|()| path),
            ));
        }
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
        let source = astro::TranscriptSource::File(path.into());
        let (chart, _) = astro::build_reading(&input, source, |_| {}).unwrap();
        Model {
            screen: Screen::Reading(Reading::new(Box::new(chart), "reading.html".into())),
            ..Model::default()
        }
    }
}
