# Natal Reading Indexer — Build Brief

## Goal
Turn a recorded birth-chart reading (≈1h call) into a **single, self-contained HTML artifact** where the astrologer's *verbatim* words are filed under the chart elements they refer to — planets, signs, houses, and aspects (edges) — and can be filtered by clicking a **computed chart wheel** or element toggles. One excerpt may attach to many elements.

## Non-negotiables
- **All interpretive content is the astrologer's.** No machine-authored prose anywhere. The model only *routes* verbatim transcript spans to chart elements.
- **OSI-permissive licenses only.** No AGPL/copyleft in the dependency chain (rules out Swiss Ephemeris family — see below).
- **Local-first / offline.** Local transcription + local model; the output HTML makes no network calls and runs from `file://`.
- **Portable output.** One HTML file with data embedded (no fetch of local JSON — fails under `file://`).

## Pipeline
Four stages, all local. The `ChartData` object (below) is the contract between them.

1. **Transcribe** — existing stack: Parakeet-TDT via MLX + Silero VAD. Preserve word/segment timestamps.
2. **Compute chart** — from birth data (date, time, place → lat/lon + *historical* timezone). Fills `meta`, `axes`, `houseCusps`, `planets`, `aspects`. See options.
3. **Route/tag** — local LLM (e.g. Qwen2.5-7B, Ministral-8B) as **closed-set classifier**, not a generator. Vocabulary = the tag-ids derived from stage 2. For each span it emits `{ span offsets, tag-ids }` — never text.
   - **Verify gate (total):** reject any span whose `text` is not a substring of the transcript; reject any tag-id not in the chart vocabulary. This preserves an unbroken provenance chain.
4. **Emit** — inject the assembled `ChartData` into the HTML template (the prototype renderer).

## Chart computation — permissive options
Pick based on where computation should live. Both are offline-capable.

- **In-browser (best fit for "app takes inputs, draws chart"): `circular-natal-horoscope-js`** — Unlicense (public domain). Analytic Moshier ephemeris (no data files, fully offline). Returns bodies, Ascendant, MC, house cusps for 7 systems incl. **Whole Sign**, aspects, retrograde, from date/time/lat/lon. Deps: `moment`, `moment-timezone`, `tz-lookup`. Caveats: unmaintained since 2021; arcminute accuracy (fine for astrology).
  - Repo: https://github.com/0xStarcat/CircularNatalHoroscopeJS
- **Python pipeline: `skyfield`** — MIT. Positions from a JPL DE440s ephemeris (public domain, offline after one download). Whole Sign / Equal cusps are trivial; ASC/MC from local sidereal time + obliquity; Placidus/quadrant systems need the iterative algorithm.
- **Avoid:** `pyswisseph` and `libephemeris` — both **AGPL-3.0** (copyleft), which would infect the project.
- **Geocoding + timezone:** place → lat/lon via an offline GeoNames gazetteer; historical tz at the birth instant via the IANA tz database (bundled in `moment-timezone`).

## Data contract (`ChartData`)
Embedded in the HTML as a `const DATA = {...}`. Tag-id convention is the join key across all stages: `planet:sun`, `sign:leo`, `house:5`, `aspect:sun-moon`.

```ts
interface ChartData {
  meta: { name: string; born: string; place: string; system: string; zodiac: string };
  axes: { asc: number; mc: number };          // ecliptic longitude, deg; 0 = 0° Aries
  houseCusps: number[];                        // 12 longitudes; cusp[0] = 1st-house cusp
  planets: Body[];                             // include Ascendant as a point (glyph "AC")
  signs: Ref[];                                // wheel draws all 12; list may hold only those referenced
  houses: HouseRef[];
  aspects: Aspect[];
  excerpts: Excerpt[];
}
interface Body     { id: string; glyph: string; name: string; lon: number; house: number }
interface Ref      { id: string; glyph: string; name: string }
interface HouseRef { id: string; label: string; name: string }         // label = roman numeral
interface Aspect   { id: string; glyph: string; name: string; a: string; b: string } // a,b = planet ids
interface Excerpt  {
  id: string;
  time: string;                // "HH:MM:SS" anchor into the recording
  span: [number, number];      // char offsets into transcript — provenance
  text: string;                // VERBATIM; must equal transcript.slice(...span)
  tags: string[];              // tag-ids; each must exist in the vocab above
}
```
Note: the prototype excerpts don't yet carry `span` — add it in the real build so the Verify gate can enforce provenance.

## Viewer (prototype exists: `reading.html`)
Reference renderer — data-driven, single file, no external calls. Use as the template stage-4 fills.
- Computed **SVG wheel**: planets at true longitudes, house-cusp spokes, ASC/MC axes, aspect chords across centre. Wheel doubles as the filter surface (tap planet glyph / sign band / aspect chord).
- Token bands (planets · signs · houses · aspects) as the keyboard-accessible filter equivalent.
- **Any / All** match mode (All = passages touching every selected element at once).
- Header states provenance explicitly; excerpts render as blockquotes with clickable tag chips.
- Aesthetic: engraved star-atlas — midnight indigo field, category colors (planets=brass, signs=verdigris, houses=steel, aspects=oxblood).

## Open question (resolve before wiring computation)
"**Seasonal mode**" from the reading: confirm it means the ordinary **tropical zodiac** (signs anchored to equinoxes/solstices — current assumption) vs a **sidereal** frame needing an ayanamsha. It changes the ephemeris config.

## Suggested build order
1. Lock `ChartData` as a typed contract + a JSON-schema validator (used by the Verify gate).
2. Chart module: birth inputs → chart fields (start with Whole Sign).
3. Router module: prompt + closed-set tagging + Verify gate → `excerpts`.
4. Transcription glue → transcript + timestamps feeding stages 2–3.
5. Template generator: inject `ChartData` into `reading.html` → final artifact.
