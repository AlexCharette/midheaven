use astro::chart::{BirthInput, compute_chart, parse_time};
use astro::{TranscriptSource, build_reading, emit, geo};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// An offline astrology workspace — birth-chart computation plus routing of
/// verbatim reading-transcript excerpts to chart elements, emitted as one
/// self-contained HTML artifact.
#[derive(Parser)]
#[command(name = "astro", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
        #[arg(long, required_unless_present = "audio", conflicts_with = "audio")]
        transcript: Option<PathBuf>,
        /// WAV recording to transcribe first (requires --model).
        #[arg(long, requires = "model")]
        audio: Option<PathBuf>,
        /// ggml whisper model file for --audio.
        #[arg(long)]
        model: Option<PathBuf>,
        /// Output HTML path.
        #[arg(long, default_value = "reading.html")]
        out: PathBuf,
        /// Also engrave a PDF (cream-paper rendition) to this path.
        #[arg(long)]
        pdf: Option<PathBuf>,
        /// PDF page size: a4 or letter.
        #[arg(long, default_value = "a4")]
        page_size: String,
    },
    /// Transcribe a WAV recording to timestamped JSONL (local whisper.cpp).
    Transcribe {
        /// WAV file, any sample rate/channels. For m4a/mp3 convert first:
        /// ffmpeg -i call.m4a -ar 16000 -ac 1 call.wav
        #[arg(long)]
        audio: PathBuf,
        /// ggml whisper model file (e.g. ggml-small.bin).
        #[arg(long)]
        model: PathBuf,
        /// Output JSONL path (stdout when omitted).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Search the offline gazetteer (what a place query will resolve to).
    Places {
        /// Query, e.g. "portland, oregon" — quotes optional, words are joined.
        query: Vec<String>,
    },
}

#[derive(Args)]
struct BirthArgs {
    /// Name shown on the chart.
    #[arg(long, default_value = astro::chart::DEFAULT_NAME)]
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
        let time = parse_time(&self.time)?;

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

/// The CLI's one transcription-progress protocol: banner, then a percent
/// line rewritten in place (finished with a newline at 100).
fn transcription_banner(audio: &std::path::Path) {
    eprintln!("transcribing {} (this can take a while)…", audio.display());
}

fn cli_progress(pct: i32) {
    eprint!("\r  {pct:>3}%");
    if pct >= 100 {
        eprintln!();
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
        Command::Chart(birth) => {
            let input = birth.into_input()?;
            let chart = compute_chart(&input)?;
            println!("{}", serde_json::to_string_pretty(&chart).map_err(|e| e.to_string())?);
        }
        Command::Build { birth, transcript, audio, model, out, pdf, page_size } => {
            let page_size = astro::pdf::PageSize::parse(&page_size)?;
            let input = birth.into_input()?;
            let source = match (transcript, audio) {
                (Some(path), _) => TranscriptSource::File(path),
                (None, Some(wav)) => {
                    let Some(model) = model else {
                        return Err("--audio requires --model".into());
                    };
                    transcription_banner(&wav);
                    TranscriptSource::Audio { wav, model }
                }
                (None, None) => TranscriptSource::None, // clap prevents this
            };
            let (chart, n_routed) = build_reading(&input, source, cli_progress)?;
            emit::write_artifact(&chart, &out)?;
            if let Some(pdf_out) = pdf {
                astro::pdf::write_pdf(&chart, page_size, &pdf_out)?;
                eprintln!("wrote {}", pdf_out.display());
            }
            eprintln!(
                "chart: {} planets, {} aspects · router: {} spans → {} excerpts past verify gate",
                chart.planets.len(),
                chart.aspects.len(),
                n_routed,
                chart.excerpts.len()
            );
            eprintln!("wrote {}", out.display());
        }
        Command::Transcribe { audio, model, out } => {
            transcription_banner(&audio);
            let segments = astro::transcribe::transcribe(&audio, &model, cli_progress)?;
            eprintln!("\r  done — {} segments", segments.len());
            let jsonl = astro::transcribe::to_jsonl(&segments);
            match out {
                Some(path) => {
                    std::fs::write(&path, &jsonl)
                        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                    eprintln!("wrote {} segments to {}", segments.len(), path.display());
                }
                None => print!("{jsonl}"),
            }
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
