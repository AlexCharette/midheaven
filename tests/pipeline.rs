//! End-to-end pipeline test through the public library API — the same path
//! the CLI (and the future TUI) drives: compute → route → verify → emit.

use astro::chart::{BirthInput, compute_chart};
use astro::emit::emit;
use astro::i18n::Locale;
use astro::route::{LexiconRouter, Transcript, index_transcript};
use astro::{TranscriptSource, build_reading};

#[test]
fn transcript_to_artifact() {
    let input = BirthInput {
        name: "Integration".into(),
        date: "1990-07-13".parse().unwrap(),
        time: "14:30:00".parse().unwrap(),
        lat: 52.52,
        lon: 13.405,
        tz: "Europe/Berlin".parse().unwrap(),
        place: "Berlin, Germany".into(),
        locale: Locale::En,
        house_system: astro::chart::systems::house_system("whole-sign"),
        ayanamsa: None,
    };
    let mut chart = compute_chart(&input).expect("chart computes");

    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/transcript.jsonl"
    ))
    .expect("example transcript present");
    let transcript = Transcript::load(&raw);

    let vocab = chart.vocab();
    let router = LexiconRouter::new(&vocab, &chart.aspects, Locale::En);
    index_transcript(&mut chart, &transcript, &router);

    // Nine routed sentences coalesce into five passages (consecutive
    // passages sharing a tag merge); spot-check known tags.
    assert_eq!(chart.excerpts.len(), 5);
    let first = &chart.excerpts[0];
    assert!(first.tags.contains(&"planet:sun".to_string()));
    assert!(first.tags.contains(&"sign:cancer".to_string()));
    assert!(first.tags.contains(&"house:10".to_string()));
    assert_eq!(first.time, "00:00:42");
    // Provenance invariant holds end to end.
    for ex in &chart.excerpts {
        assert_eq!(ex.text, &transcript.text[ex.span[0]..ex.span[1]]);
        assert!(ex.tags.iter().all(|t| vocab.contains(t)));
    }

    let html = emit(&chart).expect("emit succeeds");
    assert!(html.contains("const DATA = {"));
    assert!(!html.contains("/*__DATA__*/null"));
    assert!(html.contains("Integration"));

    // The one-call orchestrator walks the same path.
    let source = TranscriptSource::File(
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/transcript.jsonl").into(),
    );
    let (via_lib, report) = build_reading(&input, source, |_| {}).expect("build_reading");
    assert_eq!(via_lib.excerpts.len(), chart.excerpts.len());
    assert_eq!(report.n_routed, 9, "router emits sentence-level spans before coalescing");
    assert!(report.warnings.is_empty(), "clean chart routes without warnings: {:?}", report.warnings);

    // The transcript comes back for callers that archive the reading — the
    // desktop writes exactly this into the library folder. Verbatim, and named
    // after the source's kind, so a saved passage's span still points at it.
    let archived = report.transcript.expect("a file build archives its transcript");
    assert_eq!(archived.filename, "transcript.jsonl");
    assert_eq!(archived.contents, raw, "archived verbatim");
    for ex in &via_lib.excerpts {
        let reparsed = Transcript::load(&archived.contents);
        assert_eq!(
            ex.text,
            &reparsed.text[ex.span[0]..ex.span[1]],
            "spans must still resolve against the archived transcript"
        );
    }
}

/// A build with no transcript archives nothing — the chart is the whole
/// reading, and the library has no transcript file to write.
#[test]
fn a_chart_only_build_archives_no_transcript() {
    let input = BirthInput {
        name: "No Words".into(),
        date: "1990-07-13".parse().unwrap(),
        time: "14:30:00".parse().unwrap(),
        lat: 52.52,
        lon: 13.405,
        tz: "Europe/Berlin".parse().unwrap(),
        place: "Berlin, Germany".into(),
        locale: Locale::En,
        house_system: astro::chart::systems::house_system("whole-sign"),
        ayanamsa: None,
    };
    let (chart, report) =
        build_reading(&input, TranscriptSource::None, |_| {}).expect("build_reading");
    assert!(chart.excerpts.is_empty());
    assert_eq!(report.n_routed, 0);
    assert_eq!(report.transcript, None);
}

/// A plain-text transcript is archived under its own extension, not forced to
/// `.jsonl` — the desktop's library folder shows the words the way they arrived.
#[test]
fn a_text_transcript_keeps_its_extension() {
    let dir = std::env::temp_dir().join("midheaven-pipeline-text-transcript");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("session.txt");
    let words = "The sun sits in cancer. It rules the tenth house.";
    std::fs::write(&path, words).expect("write transcript");

    let input = BirthInput {
        name: "Plain Text".into(),
        date: "1990-07-13".parse().unwrap(),
        time: "14:30:00".parse().unwrap(),
        lat: 52.52,
        lon: 13.405,
        tz: "Europe/Berlin".parse().unwrap(),
        place: "Berlin, Germany".into(),
        locale: Locale::En,
        house_system: astro::chart::systems::house_system("whole-sign"),
        ayanamsa: None,
    };
    let (chart, report) = build_reading(&input, TranscriptSource::File(path), |_| {})
        .expect("build_reading");

    let archived = report.transcript.expect("archived");
    assert_eq!(archived.filename, "transcript.txt");
    assert_eq!(archived.contents, words);
    assert!(!chart.excerpts.is_empty(), "the words name chart elements");
    // Plain text carries no time anchors, so passages have no time.
    assert!(chart.excerpts.iter().all(|ex| ex.time.is_empty()));

    std::fs::remove_dir_all(&dir).ok();
}
