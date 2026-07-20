mod tui;

use astro::chart::{BirthInput, compute_chart};
use astro::route::{LexiconRouter, Transcript, index_transcript};
use astro::{emit, geo};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Natal reading indexer — offline birth-chart computation plus routing of
/// verbatim reading-transcript excerpts to chart elements, emitted as one
/// self-contained HTML artifact. Run without a subcommand for the TUI.
#[derive(Parser)]
#[command(name = "astro", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Compute the chart and print ChartData as JSON (no transcript).
    Chart(BirthArgs),
    /// Full pipeline: chart + transcript routing → HTML artifact.
    Build {
        #[command(flatten)]
        birth: BirthArgs,
        /// Transcript file: plain .txt, or JSONL segments {"start", "text"}.
        #[arg(long)]
        transcript: PathBuf,
        /// Output HTML path.
        #[arg(long, default_value = "reading.html")]
        out: PathBuf,
    },
    /// Search the offline gazetteer (what a place query will resolve to).
    Places {
        /// Query, e.g. "portland, oregon" — quotes optional, words are joined.
        query: Vec<String>,
    },
    /// Open the interactive terminal interface (also the bare default).
    Tui,
}

#[derive(Args)]
struct BirthArgs {
    /// Name shown on the chart.
    #[arg(long, default_value = "Anonymous")]
    name: String,
    /// Birth date, YYYY-MM-DD.
    #[arg(long)]
    date: chrono::NaiveDate,
    /// Local birth time, HH:MM or HH:MM:SS.
    #[arg(long)]
    time: String,
    /// Birth place query against the offline gazetteer, e.g. "berlin" or
    /// "portland, oregon". Resolves latitude, longitude, and timezone.
    #[arg(long)]
    place: Option<String>,
    /// GeoNames id of the birth place (exact; see `astro places`).
    #[arg(long, conflicts_with = "place")]
    place_id: Option<u32>,
    /// Latitude in decimal degrees (north positive). Overrides --place.
    #[arg(long)]
    lat: Option<f64>,
    /// Longitude in decimal degrees (east positive). Overrides --place.
    #[arg(long)]
    lon: Option<f64>,
    /// IANA timezone of the birth place, e.g. Europe/Berlin. Overrides --place.
    #[arg(long)]
    tz: Option<chrono_tz::Tz>,
    /// Place label shown on the chart header (defaults to the resolved place).
    #[arg(long)]
    place_label: Option<String>,
}

impl BirthArgs {
    fn into_input(self) -> Result<BirthInput, String> {
        let time = self
            .time
            .parse()
            .or_else(|_| format!("{}:00", self.time).parse())
            .map_err(|e| format!("invalid --time {:?}: {e}", self.time))?;

        let resolved: Option<&'static geo::Place> = if let Some(id) = self.place_id {
            Some(geo::by_id(id).ok_or(format!("no place with geonames id {id}"))?)
        } else if let Some(query) = &self.place {
            match geo::resolve(query) {
                geo::Resolution::Match(p) => {
                    eprintln!(
                        "place: {} → {:.4}{} {:.4}{} · {}",
                        p.label(),
                        p.lat.abs(), if p.lat >= 0.0 { "N" } else { "S" },
                        p.lon.abs(), if p.lon >= 0.0 { "E" } else { "W" },
                        p.tz
                    );
                    Some(p)
                }
                geo::Resolution::Ambiguous(candidates) => {
                    eprintln!("--place {query:?} is ambiguous; candidates:");
                    print_places(&candidates);
                    return Err(format!(
                        "narrow it with a qualifier (e.g. --place \"{query}, {}\") or use --place-id",
                        candidates[0].cc.to_lowercase()
                    ));
                }
                geo::Resolution::NotFound => {
                    return Err(format!(
                        "no place matches {query:?} in the offline gazetteer; \
                         try `astro places <query>` or pass --lat/--lon/--tz"
                    ));
                }
            }
        } else {
            None
        };

        let field = |manual: Option<f64>, from_place: Option<f64>, flag: &str| {
            manual.or(from_place).ok_or(format!("--{flag} is required unless --place/--place-id is given"))
        };
        Ok(BirthInput {
            name: self.name,
            date: self.date,
            time,
            lat: field(self.lat, resolved.map(|p| p.lat), "lat")?,
            lon: field(self.lon, resolved.map(|p| p.lon), "lon")?,
            tz: self
                .tz
                .or(resolved.map(|p| p.tz))
                .ok_or("--tz is required unless --place/--place-id is given")?,
            place: self
                .place_label
                .or(resolved.map(|p| p.label()))
                .unwrap_or_default(),
        })
    }
}

fn print_places(places: &[&geo::Place]) {
    for (i, p) in places.iter().enumerate() {
        eprintln!(
            "  {:>2}. {:<40} {:>9.4} {:>9.4}  {:<22} id {}",
            i + 1,
            p.label(),
            p.lat,
            p.lon,
            p.tz.to_string(),
            p.id
        );
    }
}

fn run() -> Result<(), String> {
    match Cli::parse().command {
        None | Some(Command::Tui) => tui::run(),
        Some(command) => run_command(command),
    }
}

fn run_command(command: Command) -> Result<(), String> {
    match command {
        Command::Tui => unreachable!("handled in run"),
        Command::Chart(birth) => {
            let input = birth.into_input()?;
            let chart = compute_chart(&input)?;
            println!("{}", serde_json::to_string_pretty(&chart).map_err(|e| e.to_string())?);
        }
        Command::Build { birth, transcript, out } => {
            let input = birth.into_input()?;
            let mut chart = compute_chart(&input)?;

            let raw = std::fs::read_to_string(&transcript)
                .map_err(|e| format!("cannot read {}: {e}", transcript.display()))?;
            let transcript = Transcript::load(&raw);

            let router = LexiconRouter::new(&chart.vocab(), &chart.aspects);
            let n_routed = index_transcript(&mut chart, &transcript, &router);

            let html = emit::emit(&chart)?;
            std::fs::write(&out, &html).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
            eprintln!(
                "chart: {} planets, {} aspects · router: {} spans → {} excerpts past verify gate",
                chart.planets.len(),
                chart.aspects.len(),
                n_routed,
                chart.excerpts.len()
            );
            eprintln!("wrote {}", out.display());
        }
        Command::Places { query } => {
            let query = query.join(" ");
            let hits = geo::search(&query, 10);
            if hits.is_empty() {
                return Err(format!("no place matches {query:?}"));
            }
            print_places(&hits);
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
