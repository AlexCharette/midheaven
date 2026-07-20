//! The TEA runtime: draw → poll → decode → update → execute commands →
//! feed results back as messages. All rendering lives in `view`, all state
//! transitions in `update`; this module owns the terminal and the effects.

mod model;
mod theme;
mod update;
mod view;
mod wheel;


use astro::route::{LexiconRouter, Transcript, index_transcript};
use model::{Model, Screen};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;
use update::{Cmd, Msg, decode, update};

pub fn run() -> Result<(), String> {
    // The gazetteer's one-time parse happens before raw mode, so the first
    // keystroke in the place field is instant.
    eprintln!("consulting the gazetteer…");
    let _ = astro::geo::search("x", 1);

    let mut terminal = ratatui::init();
    let mut model = Model::default();
    let result = loop {
        if let Err(e) = terminal.draw(|f| view::view(&model, f)) {
            break Err(e.to_string());
        }
        match event::poll(Duration::from_millis(250)) {
            Ok(false) => {}
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if let Some(msg) = decode(&model, key) {
                        dispatch(&mut model, msg);
                    }
                }
                Ok(_) => {} // resize etc. — the next draw handles it
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
    let mut queue = vec![msg];
    while !queue.is_empty() {
        let mut cmds = Vec::new();
        for msg in queue.drain(..) {
            cmds.extend(update(model, msg));
        }
        queue.extend(cmds.into_iter().filter_map(|c| execute(c, model)));
    }
}

/// The effect interpreter: the only place the TUI touches the filesystem or
/// the pipeline.
fn execute(cmd: Cmd, model: &Model) -> Option<Msg> {
    match cmd {
        Cmd::Build { input, transcript } => Some(Msg::Built(build(input, transcript))),
        Cmd::Emit => {
            let Screen::Reading(reading) = &model.screen else {
                return None;
            };
            Some(Msg::Emitted(
                astro::emit::emit(&reading.chart)
                    .and_then(|html| {
                        std::fs::write(&reading.out, html).map_err(|e| e.to_string())
                    })
                    .map(|()| reading.out.clone()),
            ))
        }
    }
}

fn build(
    input: astro::chart::BirthInput,
    transcript: Option<std::path::PathBuf>,
) -> Result<Box<astro::contract::ChartData>, String> {
    let mut chart = astro::chart::compute_chart(&input)?;
    if let Some(path) = transcript {
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let transcript = Transcript::load(&raw);
        let router = LexiconRouter::new(&chart.vocab(), &chart.aspects);
        index_transcript(&mut chart, &transcript, &router);
    }
    Ok(Box::new(chart))
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
        let mut chart = astro::chart::compute_chart(&input).unwrap();
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/transcript.jsonl"
        ))
        .unwrap();
        let transcript = astro::route::Transcript::load(&raw);
        let router = astro::route::LexiconRouter::new(&chart.vocab(), &chart.aspects);
        astro::route::index_transcript(&mut chart, &transcript, &router);
        Model {
            screen: Screen::Reading(Reading::new(Box::new(chart), "reading.html".into())),
            ..Model::default()
        }
    }
}
