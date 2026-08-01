use astro::chart::{BirthInput, compute_chart_reporting, parse_time, systems};
use astro::i18n::Locale;
use astro::{ClassifyError, TranscriptSource, build_reading, emit, geo};
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
        /// Also engrave a PDF to this path.
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
        /// Whisper language hint (e.g. ru); omit to auto-detect. Russian needs
        /// a multilingual model, not an English-only (.en) one.
        #[arg(long)]
        lang: Option<String>,
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
    /// Name shown on the chart. Blank falls back to the locale's anonymous
    /// persona (the same default every frontend uses).
    #[arg(long, default_value = "")]
    name: String,
    /// Reading language: en or ru. Drives element names and the router's
    /// match terms (and, for --audio, the whisper language hint).
    #[arg(long, default_value = "en")]
    lang: String,
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
    /// House system: whole-sign, placidus, koch, equal, regiomontanus,
    /// campanus, or porphyry.
    #[arg(long, default_value = "whole-sign")]
    house_system: String,
    /// Zodiac: tropical or sidereal.
    #[arg(long, default_value = "tropical")]
    zodiac: String,
    /// Ayanamsa used when --zodiac sidereal: lahiri, fagan-bradley, kp, raman,
    /// or true-chitra.
    #[arg(long, default_value = "lahiri")]
    ayanamsa: String,
}

impl BirthArgs {
    /// Build the birth input. Returns the input plus an optional place-resolution
    /// notice for the caller to print — resolution stays a pure conversion, the
    /// CLI presentation lives in `run`.
    fn into_input(self) -> Result<(BirthInput, Option<String>), String> {
        let time = parse_time(&self.time)?;

        let mut notice = None;
        let resolved: Option<&'static geo::Place> = if let Some(id) = self.place_id {
            Some(geo::by_id(id).ok_or(format!("no place with geonames id {id}"))?)
        } else if let Some(query) = &self.place {
            match geo::resolve(query) {
                geo::Resolution::Match(p) => {
                    notice = Some(format!(
                        "place: {} → {:.4}{} {:.4}{} · {}",
                        p.label(),
                        p.lat.abs(), if p.lat >= 0.0 { "N" } else { "S" },
                        p.lon.abs(), if p.lon >= 0.0 { "E" } else { "W" },
                        p.tz
                    ));
                    Some(p)
                }
                geo::Resolution::Ambiguous(candidates) => {
                    return Err(format!(
                        "--place {query:?} is ambiguous; candidates:\n{}narrow it with a \
                         qualifier (e.g. --place \"{query}, {}\") or use --place-id",
                        format_places(&candidates),
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

        let locale = Locale::parse(&self.lang);

        // The CLI's flags are one tier; there are no preferences behind them.
        let calc = systems::resolve(
            systems::Codes::new(
                Some(&self.house_system),
                Some(&self.zodiac),
                Some(&self.ayanamsa),
            ),
            systems::Codes::default(),
        )?;

        // A resolved place gives every field; the flags then override whichever
        // of them the caller stated. Without a place there is nothing to
        // override, so all three coordinates are required.
        let required = |flag: &str| format!("--{flag} is required unless --place/--place-id is given");
        let mut input = match resolved {
            Some(place) => astro::birth_at_place(
                &self.name,
                self.date,
                time,
                place,
                locale,
                calc.house_system,
                calc.ayanamsa,
            ),
            None => BirthInput {
                name: locale.name_or_anonymous(&self.name).to_string(),
                date: self.date,
                time,
                lat: self.lat.ok_or_else(|| required("lat"))?,
                lon: self.lon.ok_or_else(|| required("lon"))?,
                tz: self.tz.ok_or_else(|| required("tz"))?,
                place: String::new(),
                locale,
                house_system: calc.house_system,
                ayanamsa: calc.ayanamsa,
            },
        };
        if let Some(lat) = self.lat {
            input.lat = lat;
        }
        if let Some(lon) = self.lon {
            input.lon = lon;
        }
        if let Some(tz) = self.tz {
            input.tz = tz;
        }
        if let Some(label) = self.place_label {
            input.place = label;
        }
        Ok((input, notice))
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

fn format_places(places: &[&geo::Place]) -> String {
    let mut out = String::new();
    for (i, p) in places.iter().enumerate() {
        out.push_str(&format!(
            "  {:>2}. {:<40} {:>9.4} {:>9.4}  {:<22} id {}\n",
            i + 1,
            p.label(),
            p.lat,
            p.lon,
            p.tz.to_string(),
            p.id
        ));
    }
    out
}

/// What a command did — everything `main` prints, and nothing about how.
///
/// Returning this rather than printing along the way is the whole seam: the
/// notice, the warnings, the summary and the candidate table used to leave only
/// through `eprintln!`, so nothing could observe them and `run` could not be
/// called at all — it read `std::env::args()` itself.
#[derive(Debug, PartialEq)]
enum Report {
    Chart { notice: Option<String>, warnings: Vec<String>, json: String },
    Built { notice: Option<String>, warnings: Vec<String>, summary: String, wrote: Vec<PathBuf> },
    Transcribed { segments: usize, jsonl: Option<String>, wrote: Option<PathBuf> },
    Places(String),
}

/// Everything a report says, on the streams it belongs on: the artifact itself
/// on stdout, the running commentary on stderr.
fn print(report: Report) {
    let note = |notice: Option<String>, warnings: Vec<String>| {
        if let Some(n) = notice {
            eprintln!("{n}");
        }
        for w in warnings {
            eprintln!("warning: {w}");
        }
    };
    match report {
        Report::Chart { notice, warnings, json } => {
            note(notice, warnings);
            println!("{json}");
        }
        Report::Built { notice, warnings, summary, wrote } => {
            note(notice, warnings);
            eprintln!("{summary}");
            for path in wrote {
                eprintln!("wrote {}", path.display());
            }
        }
        Report::Transcribed { segments, jsonl, wrote } => {
            eprintln!("\r  done — {segments} segments");
            if let Some(path) = wrote {
                eprintln!("wrote {segments} segments to {}", path.display());
            }
            if let Some(jsonl) = jsonl {
                print!("{jsonl}");
            }
        }
        Report::Places(table) => eprint!("{table}"),
    }
}

fn run(cli: Cli) -> Result<Report, String> {
    match cli.command {
        Command::Chart(birth) => {
            let (input, notice) = birth.into_input()?;
            let (chart, warnings) = compute_chart_reporting(&input)?;
            let json = serde_json::to_string_pretty(&chart).map_err(|e| e.to_string())?;
            Ok(Report::Chart { notice, warnings, json })
        }
        Command::Build { birth, transcript, audio, model, out, pdf, page_size } => {
            let page_size = astro::pdf::PageSize::parse(&page_size)?;
            let (input, notice) = birth.into_input()?;
            // clap guarantees exactly one of --transcript / --audio is present
            // (required_unless_present + conflicts_with), and --audio requires
            // --model, so the model-missing arm mirrors ClassifyError.
            let source = if let Some(path) = transcript {
                TranscriptSource::File(path)
            } else if let Some(wav) = audio {
                let model = model.ok_or_else(|| ClassifyError::ModelRequired.to_string())?;
                transcription_banner(&wav);
                TranscriptSource::Audio { wav, model }
            } else {
                unreachable!("clap requires --transcript or --audio")
            };
            let (chart, report) = build_reading(&input, source, cli_progress)?;
            emit::write_artifact(&chart, &out)?;
            let mut wrote = Vec::new();
            if let Some(pdf_out) = pdf {
                astro::pdf::write_pdf(&chart, page_size, &pdf_out)?;
                wrote.push(pdf_out);
            }
            wrote.push(out);
            let summary = format!(
                "chart: {} planets, {} aspects · router: {} spans → {} excerpts past verify gate",
                chart.planets.len(),
                chart.aspects.len(),
                report.n_routed,
                chart.excerpts.len()
            );
            Ok(Report::Built { notice, warnings: report.warnings, summary, wrote })
        }
        Command::Transcribe { audio, model, lang, out } => {
            transcription_banner(&audio);
            let segments =
                astro::transcribe::transcribe(&audio, &model, lang.as_deref(), cli_progress)?;
            let jsonl = astro::transcribe::to_jsonl(&segments);
            match out {
                Some(path) => {
                    std::fs::write(&path, &jsonl)
                        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                    Ok(Report::Transcribed { segments: segments.len(), jsonl: None, wrote: Some(path) })
                }
                None => Ok(Report::Transcribed { segments: segments.len(), jsonl: Some(jsonl), wrote: None }),
            }
        }
        Command::Places { query } => {
            let query = query.join(" ");
            let hits = geo::search(&query, 10);
            if hits.is_empty() {
                return Err(format!("no place matches {query:?}"));
            }
            Ok(Report::Places(format_places(&hits)))
        }
    }
}

fn main() {
    match run(Cli::parse()) {
        Ok(report) => print(report),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seam: a command line as a person would type it. `run` used to read
    /// `std::env::args()` itself, so nothing below was reachable.
    ///
    /// `try_parse_from` rather than `parse_from` because the latter exits the
    /// process on a malformed line — right for a person at a terminal, useless
    /// in a test.
    fn run_argv(args: &[&str]) -> Result<Report, String> {
        let cli = Cli::try_parse_from(std::iter::once("astro").chain(args.iter().copied()))
            .map_err(|e| e.to_string())?;
        run(cli)
    }

    fn chart_argv(extra: &[&str]) -> Vec<String> {
        let mut v: Vec<String> =
            ["chart", "--date", "1990-07-13", "--time", "14:30"].iter().map(|s| s.to_string()).collect();
        v.extend(extra.iter().map(|s| s.to_string()));
        v
    }

    fn chart(extra: &[&str]) -> Result<Report, String> {
        let owned = chart_argv(extra);
        run_argv(&owned.iter().map(String::as_str).collect::<Vec<_>>())
    }

    /// The computed chart, for assertions about what the flags resolved to.
    fn computed(extra: &[&str]) -> serde_json::Value {
        match chart(extra).expect("the chart computes") {
            Report::Chart { json, .. } => serde_json::from_str(&json).expect("valid JSON"),
            other => panic!("expected a chart report, got {other:?}"),
        }
    }

    #[test]
    fn a_place_resolves_and_says_what_it_resolved_to() {
        let Report::Chart { notice, .. } = chart(&["--place", "berlin"]).unwrap() else {
            panic!("expected a chart");
        };
        let notice = notice.expect("a resolved place is confirmed to the caller");
        assert!(notice.contains("Berlin"), "{notice}");
        // Hemisphere letters rather than signs — Berlin is north and east.
        assert!(notice.contains('N') && notice.contains('E'), "{notice}");
    }

    #[test]
    fn a_southern_western_place_says_so() {
        let Report::Chart { notice, .. } = chart(&["--place", "montevideo"]).unwrap() else {
            panic!("expected a chart");
        };
        let notice = notice.unwrap();
        assert!(notice.contains('S') && notice.contains('W'), "{notice}");
    }

    /// The gazetteer's ambiguity resolution has exactly one caller in the
    /// workspace — this one. The desktop disambiguates in its typeahead.
    #[test]
    fn an_ambiguous_place_lists_the_candidates_and_says_how_to_narrow() {
        let err = chart(&["--place", "springfield"]).unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");
        assert!(err.contains("--place-id"), "it should name the way out: {err}");
        assert!(err.contains("id "), "the candidate table carries ids: {err}");
        // The suggested qualifier is the leading candidate's country.
        assert!(err.contains("springfield, "), "{err}");
    }

    #[test]
    fn an_unknown_place_points_at_the_other_ways_in() {
        let err = chart(&["--place", "xqzzyplugh"]).unwrap_err();
        assert!(err.contains("no place matches"), "{err}");
        assert!(err.contains("astro places"), "{err}");
        assert!(err.contains("--lat"), "{err}");
    }

    #[test]
    fn coordinates_stand_in_for_a_place_and_are_each_required() {
        let ok = computed(&["--lat", "52.52", "--lon", "13.405", "--tz", "Europe/Berlin"]);
        assert_eq!(ok["meta"]["place"], "", "no place label was given, and none is invented");

        for missing in [
            vec!["--lon", "13.405", "--tz", "Europe/Berlin"],
            vec!["--lat", "52.52", "--tz", "Europe/Berlin"],
            vec!["--lat", "52.52", "--lon", "13.405"],
        ] {
            let err = chart(&missing).unwrap_err();
            assert!(err.contains("is required unless --place"), "{err}");
        }
    }

    /// A flag beats the place it would otherwise have come from — that is what
    /// makes them overrides rather than alternatives.
    #[test]
    fn a_coordinate_flag_overrides_the_resolved_place() {
        let plain = computed(&["--place", "berlin"]);
        let nudged = computed(&["--place", "berlin", "--lat", "0.0"]);
        assert_ne!(plain["axes"]["asc"], nudged["axes"]["asc"], "the chart really moved");
        // The label still comes from the place; only the coordinate changed.
        assert_eq!(plain["meta"]["place"], nudged["meta"]["place"]);
    }

    #[test]
    fn a_place_label_overrides_what_the_gazetteer_calls_it() {
        let c = computed(&["--place", "berlin", "--place-label", "the old flat"]);
        assert_eq!(c["meta"]["place"], "the old flat");
    }

    #[test]
    fn a_blank_name_becomes_the_locales_anonymous_persona() {
        let c = computed(&["--place", "berlin"]);
        assert_eq!(c["meta"]["name"], Locale::En.anonymous());
        let ru = computed(&["--place", "berlin", "--lang", "ru"]);
        assert_eq!(ru["meta"]["name"], Locale::Ru.anonymous());
        // And a name given is a name kept, whitespace trimmed.
        let named = computed(&["--place", "berlin", "--name", "  Mira Holt  "]);
        assert_eq!(named["meta"]["name"], "Mira Holt");
    }

    #[test]
    fn the_calculation_flags_reach_the_chart() {
        let c = computed(&["--place", "berlin", "--house-system", "placidus"]);
        assert_eq!(c["meta"]["house_system"], "placidus");

        let sid = computed(&["--place", "berlin", "--zodiac", "sidereal", "--ayanamsa", "raman"]);
        assert_eq!(sid["meta"]["ayanamsa"], "raman");
        // Tropical resolves no ayanamsa however the flag is set.
        let trop = computed(&["--place", "berlin", "--ayanamsa", "raman"]);
        assert!(trop["meta"]["ayanamsa"].is_null());
    }

    #[test]
    fn an_unknown_calculation_code_is_refused_rather_than_defaulted() {
        let err = chart(&["--place", "berlin", "--house-system", "nonsense"]).unwrap_err();
        assert!(err.contains("nonsense"), "{err}");
    }

    #[test]
    fn an_impossible_time_is_refused_before_anything_is_resolved() {
        let err =
            run_argv(&["chart", "--date", "1990-07-13", "--time", "25:00", "--place", "berlin"])
                .unwrap_err();
        assert!(err.contains("invalid time"), "{err}");
    }

    #[test]
    fn a_dst_gap_is_refused_with_the_moment_that_does_not_exist() {
        // 1990-03-25 02:30 never happened in Berlin.
        let err = run_argv(&[
            "chart", "--date", "1990-03-25", "--time", "02:30", "--place", "berlin",
        ])
        .unwrap_err();
        assert!(err.contains("DST gap"), "{err}");
    }

    #[test]
    fn a_dst_fold_computes_and_warns_rather_than_failing() {
        // 1990-09-30 02:30 happened twice.
        let Report::Chart { warnings, .. } = run_argv(&[
            "chart", "--date", "1990-09-30", "--time", "02:30", "--place", "berlin",
        ])
        .unwrap() else {
            panic!("expected a chart");
        };
        assert!(warnings.iter().any(|w| w.contains("ambiguous")), "{warnings:?}");
    }

    #[test]
    fn places_lists_what_a_query_would_resolve_to() {
        let Report::Places(table) = run_argv(&["places", "berlin"]).unwrap() else {
            panic!("expected a place table");
        };
        assert!(table.contains("Berlin"), "{table}");
        assert!(table.contains("Europe/Berlin"), "{table}");
        assert!(table.contains("id "), "the ids are the point — they feed --place-id");
        assert!(table.lines().count() > 1, "a search returns several");
    }

    #[test]
    fn places_says_so_when_nothing_matches() {
        let err = run_argv(&["places", "xqzzyplugh"]).unwrap_err();
        assert!(err.contains("no place matches"), "{err}");
    }

    #[test]
    fn a_multi_word_place_query_is_joined_before_searching() {
        let Report::Places(table) = run_argv(&["places", "new", "york"]).unwrap() else {
            panic!("expected a place table");
        };
        assert!(table.contains("New York"), "{table}");
    }

    #[test]
    fn a_bad_page_size_is_refused_before_the_chart_is_computed() {
        // Ordering matters: the page size is checked first, so a run that is
        // wrong in two ways reports the cheap failure.
        let err = run_argv(&[
            "build", "--date", "1990-07-13", "--time", "25:00", "--place", "berlin",
            "--transcript", "/nonexistent", "--page-size", "a3",
        ])
        .unwrap_err();
        assert!(err.contains("unknown page size"), "the cheap check comes first: {err}");
    }
}
