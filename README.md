# Midheaven — an offline astrology workspace

Offline-only birth-chart generator that allows astrologers to compute and annotate birth charts with
their commentary, either manually or from a transcript, and emits a single
self-contained HTML artifact that can be branded with a logo and sent to clients.

## Download

Installers for **macOS** (Apple Silicon + Intel) and **Windows** are on the
[download page](https://alexcharette.github.io/midheaven/) /
[releases](https://github.com/AlexCharette/midheaven/releases). The builds are
not notarized with Apple yet: on macOS, open the app once, then approve it
under System Settings → Privacy & Security (*Open Anyway*); if macOS calls a
v0.1.0 download "damaged", clear the quarantine flag with
`xattr -d com.apple.quarantine /Applications/Midheaven.app`. On Windows pick
*More info → Run anyway* when SmartScreen asks. (On Windows systems without
WebView2 the installer fetches it from Microsoft.)

## Guarantees

- **Offline** — analytic ephemeris (VSOP87A + ELP2000-82), no data files, no network;
  the output HTML runs from `file://` and makes no requests.
- **Permissive deps only** — no AGPL anywhere in the chain (`xalen-*` Apache-2.0,
  everything else MIT/Apache).

## Usage

### Transcription (built in, cross-platform)

```sh
# audio → timestamped JSONL (local whisper.cpp; CPU everywhere, Metal on macOS)
cargo run -- transcribe --audio call.wav --model ggml-small.bin --out transcript.jsonl

# or straight through: audio → routed artifact in one command
cargo run -- build --audio call.wav --model ggml-small.bin \
    --name "Sample Chart" --date 1990-07-13 --time 14:30 --place berlin
```

Models are user-supplied ggml files from
[whisper.cpp's Hugging Face repo](https://huggingface.co/ggerganov/whisper.cpp)
(`ggml-tiny.en.bin` 75 MB for quick tests → `ggml-small.bin` ~490 MB →
`ggml-large-v3-turbo.bin` ~1.6 GB for best quality). Nothing downloads at
runtime. Input is WAV (any rate/channels — downmixed and resampled
internally); for m4a/mp3 recordings convert once with
`ffmpeg -i call.m4a -ar 16000 -ac 1 call.wav` — native compressed-audio
decoding is deliberately excluded because the pure-Rust option is MPL-2.0
(copyleft, banned by the brief). Building the crate needs `cmake` and a C++
toolchain (whisper.cpp is compiled in).

### Desktop app (Tauri + SvelteKit)

```sh
cd desktop && npm install && npm run tauri dev    # develop
cd desktop && npm run tauri build                 # installable bundle
```

The same lib behind a webview: birth form with gazetteer typeahead and
native file dialogs, live SVG wheel, index + commentary filtering, background
transcription with a progress bar, and "engrave the artifact" writing the
standalone HTML. Everything computes in the native Tauri process — the
webview only renders.

**Live session recording:** when the form's model field points at a ggml
file, the reading page shows *begin transcribing* — the microphone records
natively (cpal) while the reading happens; *stop* transcribes the take and
routes its passages into the index and commentary, appending across takes
with running time anchors (a transcript loaded at build time keeps its
passages). macOS asks for microphone permission on first use
(`NSMicrophoneUsageDescription` ships in the bundle's Info.plist).

**Preferences** (the quiet link under the birth form): a models folder with a
default whisper model that prefills the form; a readings folder — once set,
every built chart auto-saves to `{name}_{date}/` there as `chart.json` with
its transcriptions alongside (`transcript.*` from the build, `take-N.jsonl`
per live take; recorded audio itself is not kept), staying current through
recording and curation; and practitioner branding — your name and logo are
engraved on every exported artifact's title plate ("prepared by …", the logo
embedded as a data URI so artifacts stay self-contained). Exports propose a
generated `{name}_{date}.html` filename. Preferences live in the OS app-config
dir; everything is optional and off until set.

**PDF export**: beside the HTML artifact, *export PDF* engraves a cream-paper
rendition — title plate (with your branding), the wheel as vector paths, an
index of positions and aspects, and the full commentary with folio anchors
and tag glyphs. Page size (A4 default / US Letter) is a preference. Fonts are
embedded and subset; the PDF, like everything else, is produced entirely
offline.

Added prerequisites for this target only: Node, and on Linux the
webkit2gtk/Tauri system packages (Windows uses the preinstalled WebView2).
This is the dependency-heavy target by design — the CLI binary remains
the zero-runtime-dependency path.

### Gazetteer data

The first `cargo build` downloads GeoNames `cities500` (~235k places; the IANA
timezone id ships in each record, so city → lat/lon/tz is one offline lookup),
strips and embeds it (~7 MB), and caches the sources under
`~/.cache/astro-geonames/`. For fully offline builds, place `cities500.zip`,
`admin1CodesASCII.txt`, and `countryInfo.txt` in a directory and set
`ASTRO_GEONAMES_DIR` to it. Runtime never touches the network.

Place data © [GeoNames](https://www.geonames.org/), licensed
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). PDF export embeds
[Libre Baskerville](https://fonts.google.com/specimen/Libre+Baskerville)
(OFL 1.1) and [DejaVu Sans](https://dejavu-fonts.github.io/) (Bitstream Vera
license) — license texts in `assets/fonts/`.

Transcripts are plain `.txt` (no timestamps) or JSONL segments
`{"start": seconds, "text": "..."}` as produced by the transcription stage
(Parakeet-TDT + Silero VAD — outside this repo).

## Pipeline / layout

The crate is a library (pipeline stages) plus a thin CLI binary (`src/main.rs`).

| Stage | Module | Notes |
|---|---|---|
| 1 Transcribe | `src/transcribe.rs` | whisper.cpp via `whisper-rs`; WAV in, timestamped segments out — external transcripts (txt/JSONL) remain first-class |
| 2 Compute | `src/chart/` | Tropical, Whole Sign; symbol tables in `chart/catalog.rs`; place lookup in `src/geo.rs` |
| 3 Route | `src/route/` | `Router` trait (`mod.rs`); `transcript.rs` parsing/segmentation, `lexicon.rs` matcher, `verify.rs` gate — the LLM router later lands as `route/llm.rs` |
| 4 Emit | `src/emit.rs` + `templates/reading.html` | injects `ChartData` at `/*__DATA__*/null` |
| 4b PDF | `src/pdf/` | krilla; the wheel as vector paths, embedded subset fonts, cream-paper palette |

`src/contract.rs` holds the `ChartData` contract shared by all stages (mirrors
the TS interface in the brief); `tests/pipeline.rs` drives the whole pipeline
through the public library API.

## Later phases (trait boundaries already in place)

- Local-LLM closed-set router (Ollama / llama.cpp) implementing `Router`

## Verify

`cargo test` — includes a golden test (Sun ≈ 0° Aries at the 2000 equinox instant),
Whole Sign cusp invariants, historical-DST conversion, Verify-gate rejection cases,
and a self-containment check on the emitted HTML.
