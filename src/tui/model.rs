//! The Model of the Elm loop: everything the view renders and update mutates.

use astro::contract::{ChartData, Excerpt};
use astro::geo::Place;
use std::collections::BTreeSet;

pub struct Model {
    pub screen: Screen,
    pub status: String,
    pub should_quit: bool,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            screen: Screen::Form(Form::default()),
            status: String::new(),
            should_quit: false,
        }
    }
}

pub enum Screen {
    Form(Form),
    Reading(Reading),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Name,
    Date,
    Time,
    Place,
    Transcript,
    Out,
}

pub const FIELDS: [Field; 6] = [
    Field::Name,
    Field::Date,
    Field::Time,
    Field::Place,
    Field::Transcript,
    Field::Out,
];

impl Field {
    pub fn label(self) -> &'static str {
        match self {
            Field::Name => "name",
            Field::Date => "born on",
            Field::Time => "at",
            Field::Place => "in",
            Field::Transcript => "transcript",
            Field::Out => "artifact",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Field::Name => "the chart holder's name",
            Field::Date => "YYYY-MM-DD",
            Field::Time => "HH:MM, local civil time",
            Field::Place => "type a city — the gazetteer will offer choices",
            Field::Transcript => "path to .txt or .jsonl (optional)",
            Field::Out => "where the HTML artifact is written",
        }
    }

    pub fn next(self) -> Field {
        let i = FIELDS.iter().position(|f| *f == self).unwrap();
        FIELDS[(i + 1) % FIELDS.len()]
    }

    pub fn prev(self) -> Field {
        let i = FIELDS.iter().position(|f| *f == self).unwrap();
        FIELDS[(i + FIELDS.len() - 1) % FIELDS.len()]
    }
}

pub struct Form {
    pub name: String,
    pub date: String,
    pub time: String,
    pub place_query: String,
    pub picked: Option<&'static Place>,
    pub transcript: String,
    pub out: String,
    pub focus: Field,
    pub suggestions: Vec<&'static Place>,
    /// Cursor within the suggestion dropdown, when it is open.
    pub sel: Option<usize>,
    pub error: Option<(Field, String)>,
}

impl Default for Form {
    fn default() -> Self {
        Form {
            name: String::new(),
            date: String::new(),
            time: String::new(),
            place_query: String::new(),
            picked: None,
            transcript: String::new(),
            out: "reading.html".to_string(),
            focus: Field::Name,
            suggestions: Vec::new(),
            sel: None,
            error: None,
        }
    }
}

impl Form {
    pub fn value(&self, field: Field) -> &str {
        match field {
            Field::Name => &self.name,
            Field::Date => &self.date,
            Field::Time => &self.time,
            Field::Place => &self.place_query,
            Field::Transcript => &self.transcript,
            Field::Out => &self.out,
        }
    }

    pub fn value_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Name => &mut self.name,
            Field::Date => &mut self.date,
            Field::Time => &mut self.time,
            Field::Place => &mut self.place_query,
            Field::Transcript => &mut self.transcript,
            Field::Out => &mut self.out,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Any,
    All,
}

/// One row of the Index of Elements.
pub struct IndexEntry {
    pub tag: String,
    pub glyph: String,
    pub name: String,
    /// Planets carry their position ("20° ♋"); other categories none.
    pub detail: String,
}

pub struct Reading {
    pub chart: Box<ChartData>,
    pub out: String,
    pub selected: BTreeSet<String>,
    pub mode: Mode,
    /// (column, row) cursor in the Index of Elements.
    pub cursor: (usize, usize),
    /// Commentary scroll offset, in rendered lines.
    pub scroll: u16,
    /// The four index columns: planets, signs, houses, aspects.
    pub columns: [Vec<IndexEntry>; 4],
}

impl Reading {
    pub fn new(chart: Box<ChartData>, out: String) -> Reading {
        let sign_glyph_at = |lon: f64| {
            let idx = (lon.rem_euclid(360.0) / 30.0) as usize;
            chart.signs[idx.min(11)].glyph.clone()
        };
        let planets = chart
            .planets
            .iter()
            .map(|p| IndexEntry {
                tag: p.id.clone(),
                glyph: p.glyph.clone(),
                name: p.name.clone(),
                detail: format!("{}° {}", (p.lon.rem_euclid(360.0) % 30.0) as u32, sign_glyph_at(p.lon)),
            })
            .collect();
        let signs = chart
            .signs
            .iter()
            .map(|s| IndexEntry {
                tag: s.id.clone(),
                glyph: s.glyph.clone(),
                name: s.name.clone(),
                detail: String::new(),
            })
            .collect();
        let houses = chart
            .houses
            .iter()
            .map(|h| IndexEntry {
                tag: h.id.clone(),
                glyph: h.label.clone(),
                name: h.name.replace(" House", ""),
                detail: String::new(),
            })
            .collect();
        let planet_name = |id: &str| {
            chart
                .planets
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.as_str())
                .unwrap_or(id)
                .to_string()
        };
        let aspects = chart
            .aspects
            .iter()
            .map(|a| IndexEntry {
                tag: a.id.clone(),
                glyph: a.glyph.clone(),
                name: format!("{} – {}", planet_name(&a.a), planet_name(&a.b)),
                detail: String::new(),
            })
            .collect();
        Reading {
            chart,
            out,
            selected: BTreeSet::new(),
            mode: Mode::Any,
            cursor: (0, 0),
            scroll: 0,
            columns: [planets, signs, houses, aspects],
        }
    }

    /// The filter semantics shared with the HTML viewer: no selection shows
    /// everything; Any = passage touches any selected tag; All = every one.
    pub fn visible(&self) -> Vec<&Excerpt> {
        self.chart
            .excerpts
            .iter()
            .filter(|ex| {
                if self.selected.is_empty() {
                    return true;
                }
                match self.mode {
                    Mode::Any => self.selected.iter().any(|t| ex.tags.contains(t)),
                    Mode::All => self.selected.iter().all(|t| ex.tags.contains(t)),
                }
            })
            .collect()
    }

    pub fn cursor_entry(&self) -> Option<&IndexEntry> {
        self.columns[self.cursor.0].get(self.cursor.1)
    }
}
