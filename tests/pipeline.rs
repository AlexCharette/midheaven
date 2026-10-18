//! End-to-end pipeline test through the public library API — the same path
//! the CLI (and the future TUI) drives: compute → route → verify → emit.

use astro::chart::compute_chart;
use astro::fixtures;
use astro::emit::emit;
use astro::i18n::Locale;
use astro::route::{Filing, LexiconRouter, Transcript, route_into_with};
use astro::{TranscriptSource, build_reading};

#[test]
fn transcript_to_artifact() {
    let input = fixtures::named("Integration");
    let mut chart = compute_chart(&input).expect("chart computes");

    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/transcript.jsonl"
    ))
    .expect("example transcript present");
    let transcript = Transcript::load(&raw);

    let router = LexiconRouter::new(Locale::En);
    route_into_with(&mut chart, &transcript, &router, Filing::Fresh);

    // Nine routed sentences coalesce into five notes (consecutive
    // notes sharing a tag merge); spot-check known tags.
    assert_eq!(chart.notes.len(), 5);
    let first = &chart.notes[0];
    assert!(first.tags.contains(&"planet:sun".to_string()));
    assert!(first.tags.contains(&"sign:cancer".to_string()));
    assert!(first.tags.contains(&"house:10".to_string()));
    assert_eq!(first.time, "00:00:42");
    // Provenance invariant holds end to end.
    for note in &chart.notes {
        // Every note here is spoken, so every span is real.
        let [s, e] = note.span.expect("a routed note carries its provenance");
        assert_eq!(note.text, &transcript.text[s..e]);
        assert!(note.tags.iter().all(|t| astro::vocab::contains(t)));
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
    assert_eq!(via_lib.notes.len(), chart.notes.len());
    assert_eq!(report.n_routed, 9, "router emits sentence-level spans before coalescing");
    assert!(report.warnings.is_empty(), "clean chart routes without warnings: {:?}", report.warnings);

    // The transcript comes back for callers that archive the reading — the
    // desktop writes exactly this into the library folder. Verbatim, and named
    // after the source's kind, so a saved note's span still points at it.
    let archived = report.transcript.expect("a file build archives its transcript");
    assert_eq!(archived.filename, "transcript.jsonl");
    assert_eq!(archived.contents, raw, "archived verbatim");
    for note in &via_lib.notes {
        let reparsed = Transcript::load(&archived.contents);
        let [s, e] = note.span.expect("a routed note carries its provenance");
        assert_eq!(
            note.text,
            &reparsed.text[s..e],
            "spans must still resolve against the archived transcript"
        );
    }
}

/// What routing the example transcript produces, stated in full, in the terms
/// that survive the move to a flat note.
///
/// Every other routing assertion in the suite reads a `span`, and `span` is
/// about to stop existing on a stored note. This one deliberately asserts on
/// **text, tags and time only** — the three things a note still has afterwards
/// — so that the routing behaviour has a witness that the removal cannot take
/// with it. If this passes before and after, routing did not change.
///
/// Updated once, at the stage that added `kind:`. That change is visible here
/// as five new tags and nothing else — every text, every time, every pair and
/// the note count came through untouched, which is the evidence that widening
/// the vocabulary did not disturb the matching underneath it.
///
/// The exact note texts are the load-bearing part. Note 1 and notes 3–4 each
/// span several sentences, including sentences that carry no tag of their own:
/// merging pulls a continuous verbatim slice, it does not join the matched
/// sentences together. That is the behaviour a careless refactor silently
/// replaces with concatenation, and reading these strings is how you catch it.
#[test]
fn routing_the_example_transcript_is_unchanged() {
    let input = fixtures::named("Integration");
    let mut chart = compute_chart(&input).expect("chart computes");
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/transcript.jsonl"
    ))
    .expect("example transcript present");
    let transcript = Transcript::load(&raw);
    let router = LexiconRouter::new(Locale::En);
    route_into_with(&mut chart, &transcript, &router, Filing::Fresh);

    let expected: [(&str, &str, &[&str], &str); 5] = [
        (
            "x1",
            "00:00:42",
            &[
                "aspect:sun-jupiter",
                "house:10",
                "kind:conjunction",
                "planet:jupiter",
                "planet:sun",
                "sign:cancer",
            ],
            "The first thing that jumps out at me is your sun in cancer sitting right at the \
             top of the chart in the tenth house. That's a career story, a public-life story, \
             and it colors everything else we'll look at today. And your sun is conjunct \
             jupiter, which is honestly one of the luckier signatures I see.",
        ),
        (
            "x2",
            "00:02:21",
            &["planet:ascendant", "sign:cancer", "sign:libra"],
            "Now, you have libra rising, so the ascendant softens all of that cancer intensity.",
        ),
        (
            "x3",
            "00:03:30",
            &[
                "aspect:sun-saturn",
                "house:4",
                "kind:opposition",
                "planet:neptune",
                "planet:saturn",
                "planet:sun",
                "planet:uranus",
                "sign:capricorn",
            ],
            "But I have to be honest with you: the sun stands in opposition to saturn across \
             the chart. Whatever you build publicly gets tested against the foundations, the \
             family, the private life. Saturn is in capricorn in your fourth house, together \
             with uranus and neptune.",
        ),
        (
            "x4",
            "00:05:44",
            &[
                "aspect:moon-venus",
                "aspect:sun-moon",
                "house:6",
                "kind:square",
                "kind:trine",
                "planet:moon",
                "planet:sun",
                "planet:venus",
                "sign:pisces",
            ],
            "Let's move to your moon in pisces, down in the sixth house. Your daily routines \
             have to leave room for the tides, for rest, or the body complains. The moon \
             squares your venus, so there can be friction between what comforts you and what \
             you actually desire. It's workable, but you have to name it. There's also a \
             lovely trine from the sun to the moon itself, water to water, which smooths a \
             lot of this out over time.",
        ),
        (
            "x5",
            "00:08:31",
            &["house:2", "planet:pluto", "sign:scorpio"],
            "One more thing before the break: pluto in scorpio in the second house.",
        ),
    ];

    assert_eq!(chart.notes.len(), expected.len(), "note count");
    for (note, (id, time, tags, text)) in chart.notes.iter().zip(expected) {
        assert_eq!(note.id, id);
        assert_eq!(note.time, time, "time anchor on {id}");
        assert_eq!(note.tags, tags, "tags on {id}");
        assert_eq!(note.text, text, "text on {id}");
    }
}

/// A build with no transcript archives nothing — the chart is the whole
/// reading, and the library has no transcript file to write.
#[test]
fn a_chart_only_build_archives_no_transcript() {
    let input = fixtures::named("No Words");
    let (chart, report) =
        build_reading(&input, TranscriptSource::None, |_| {}).expect("build_reading");
    assert!(chart.notes.is_empty());
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

    let input = fixtures::named("Plain Text");
    let (chart, report) = build_reading(&input, TranscriptSource::File(path), |_| {})
        .expect("build_reading");

    let archived = report.transcript.expect("archived");
    assert_eq!(archived.filename, "transcript.txt");
    assert_eq!(archived.contents, words);
    assert!(!chart.notes.is_empty(), "the words name chart elements");
    // Plain text carries no time anchors, so notes have no time.
    assert!(chart.notes.iter().all(|n| n.time.is_empty()));

    std::fs::remove_dir_all(&dir).ok();
}
