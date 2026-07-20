//! End-to-end pipeline test through the public library API — the same path
//! the CLI (and the future TUI) drives: compute → route → verify → emit.

use astro::chart::{BirthInput, compute_chart};
use astro::emit::emit;
use astro::route::{LexiconRouter, Transcript, index_transcript};

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
    };
    let mut chart = compute_chart(&input).expect("chart computes");

    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/transcript.jsonl"
    ))
    .expect("example transcript present");
    let transcript = Transcript::load(&raw);

    let vocab = chart.vocab();
    let router = LexiconRouter::new(&vocab, &chart.aspects);
    index_transcript(&mut chart, &transcript, &router);

    // The sample transcript routes nine passages; spot-check known tags.
    assert_eq!(chart.excerpts.len(), 9);
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
}
