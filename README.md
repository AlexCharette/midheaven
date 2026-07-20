# astro — natal reading indexer

Offline-only birth-chart generator in Rust that routes an astrologer's *verbatim*
reading-transcript excerpts to the chart elements they mention, and emits a single
self-contained HTML artifact (computed SVG wheel as filter surface + excerpt index).
Build brief: `docs/natal-reading-indexer.md`.

## Guarantees

- **Offline** — analytic ephemeris (VSOP87A + ELP2000-82), no data files, no network;
  the output HTML runs from `file://` and makes no requests.
- **Permissive deps only** — no AGPL anywhere in the chain (`xalen-*` Apache-2.0,
  everything else MIT/Apache).
- **Verbatim provenance** — the router only emits `{span, tags}`; the Verify gate
  rejects any excerpt whose text is not a byte-exact slice of the transcript or
  whose tags fall outside the chart-derived vocabulary
  (`planet:sun`, `sign:leo`, `house:5`, `aspect:sun-moon`).

## Usage

### Interactive (TUI)

```sh
cargo run            # bare invocation opens the terminal UI (same as `astro tui`)
```

An Elm-architecture ratatui interface in the artifact's engraved style: a
birth-data form with a live gazetteer typeahead, then the computed plate —
braille chart wheel, Index of Elements, and filtered Commentary.

| Keys (form) | | Keys (reading) | |
|---|---|---|---|
| tab / ↑↓ | move fields | arrows / hjkl-ish | move in the index |
| ↑↓ + enter | pick a place | space | toggle an element filter |
| F5 | compute the figure | a / c | any↔all / clear selection |
| esc | dismiss dropdown | j / k | scroll commentary |
| ctrl-c | leave | e / b / q | engrave HTML / back / quit |

### Scripting (CLI)

```sh
# full pipeline → reading.html; --place resolves lat/lon/tz offline
cargo run -- build --name "Sample Chart" \
    --date 1990-07-13 --time 14:30 --place berlin \
    --transcript examples/transcript.jsonl --out reading.html

# search the offline gazetteer (ambiguous places list candidates + ids)
cargo run -- places "portland, oregon"

# chart only → ChartData JSON on stdout; manual coordinates still work
cargo run -- chart --date 1990-07-13 --time 14:30 \
    --lat 52.52 --lon 13.405 --tz Europe/Berlin
```

Place resolution auto-picks only when safe: a single exact-name match, or one
that dominates by population ("berlin" → Berlin, DE). Otherwise it prints a
numbered candidate list — narrow with a qualifier (`--place "springfield, missouri"`)
or `--place-id <geonames id>`. `--lat/--lon/--tz` override any resolved field.

### Gazetteer data

The first `cargo build` downloads GeoNames `cities500` (~235k places; the IANA
timezone id ships in each record, so city → lat/lon/tz is one offline lookup),
strips and embeds it (~7 MB), and caches the sources under
`~/.cache/astro-geonames/`. For fully offline builds, place `cities500.zip`,
`admin1CodesASCII.txt`, and `countryInfo.txt` in a directory and set
`ASTRO_GEONAMES_DIR` to it. Runtime never touches the network.

Place data © [GeoNames](https://www.geonames.org/), licensed
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).

Transcripts are plain `.txt` (no timestamps) or JSONL segments
`{"start": seconds, "text": "..."}` as produced by the transcription stage
(Parakeet-TDT + Silero VAD — outside this repo).

## Pipeline / layout

The crate is a library (pipeline stages) plus a thin CLI binary (`src/main.rs`).

| Stage | Module | Notes |
|---|---|---|
| 2 Compute | `src/chart/` | Tropical, Whole Sign; symbol tables in `chart/catalog.rs`; place lookup in `src/geo.rs` |
| 3 Route | `src/route/` | `Router` trait (`mod.rs`); `transcript.rs` parsing/segmentation, `lexicon.rs` matcher, `verify.rs` gate — the LLM router later lands as `route/llm.rs` |
| 4 Emit | `src/emit.rs` + `templates/reading.html` | injects `ChartData` at `/*__DATA__*/null` |

`src/contract.rs` holds the `ChartData` contract shared by all stages (mirrors
the TS interface in the brief); `tests/pipeline.rs` drives the whole pipeline
through the public library API.

## Later phases (trait boundaries already in place)

- Local-LLM closed-set router (Ollama / llama.cpp) implementing `Router`
- ratatui TUI on the same core
- Offline gazetteer geocoding (place → lat/lon); sidereal mode

## Verify

`cargo test` — includes a golden test (Sun ≈ 0° Aries at the 2000 equinox instant),
Whole Sign cusp invariants, historical-DST conversion, Verify-gate rejection cases,
and a self-containment check on the emitted HTML.
