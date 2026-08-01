# Domain language

The words this project uses for its own concepts. Where code and prose have
drifted apart, the entry says which is which — the aim is one word per concept.

Seeded while introducing `src/derive.rs`; extend it rather than coining synonyms.

## Reading

A recorded birth-chart consultation: the astrologer's spoken words plus the
chart they were speaking about. The unit the whole app works on — a reading is
what gets built, curated, saved to the library, and exported.

## Chart

The computed natal chart — `contract::ChartData`. Positions, houses, aspects and
the localized names for them, plus any passages filed against them.

## Passage · Excerpt

**The same thing.** A verbatim span of the transcript filed under the chart
elements it refers to. The type is `contract::Excerpt`; the UI, the PDF and most
comments say "passage". Prefer *passage* in prose and user-facing text, *excerpt*
only when naming the type or its field.

Passages are always verbatim: the text must equal the transcript slice its span
points at. That is provenance, not a formatting preference.

## Tag id

An element identifier in the closed form `{category}:{slug}` — `planet:sun`,
`sign:leo`, `house:5`, `aspect:sun-moon`. Language-neutral, so a reading's
language never changes its tags. The four categories are fixed.

## Vocabulary

The set of tag ids a particular chart admits, derived from the chart itself
(`ChartData::vocab`). Closed: a router may only emit tags from it, and the Verify
gate rejects anything outside it.

## Verify gate

The provenance check between a router's output and a chart's passages: the span
must be a real slice of the transcript, and every tag must be in the chart's
vocabulary. Rejections surface as warnings rather than failing the build.

## Artifact

The single self-contained offline HTML file a reading exports to. Self-contained
is load-bearing: no external references at all, so it opens from `file://`
forever. Distinct from the PDF, which is a separate rendering of the same chart.

## Session

The open working state: a reading the app currently holds, and possibly a live
recorder capturing a take into it. Ten of the twenty-five backend commands
require a session; all ten refuse through one guard with one message
(`session::NO_READING`, "no chart has been built yet").

A session is never recording without a reading. It is modelled as one value on
**both** sides precisely so that cannot drift —
`desktop/src/lib/session.ts` in the webview, `desktop/src-tauri/src/session.rs`
in the backend. Each used to be separate facts: on the client, leaving a reading
mid-recording stranded a recorder whose take then landed on the next reading; on
the backend, a seven-field struct where every command re-checked the invariant
for itself.

Leaving, opening another reading, and recalculating are all **refused** while a
take is in flight rather than reconciled.

## Take

One recording appended to a live session. Takes accumulate — each is transcribed
and routed on its own, then appended, so earlier curation survives.

A take is **in flight** from the moment recording starts until its words land in
the chart, which spans two phases: **recording**, while the capture runs, and
**transcribing**, after it stops. The distinction is load-bearing — transcription
takes real time, and a second take begun in that window would be handed the same
session offset as the one still landing, so both takes' folio anchors would claim
the same stretch of the recording. A take that fails to transcribe is
**abandoned**: the reading stays open and never advances the session clock.

## Reproject

Recompute a chart's geometry for a different house system or zodiac, carrying its
passages onto the result. Only the geometry changes; the vocabulary is stable
enough that existing tags stay valid.

## Derivation · Derived field

A value computed from what a chart already carries, rather than measured
independently: which sign a longitude falls in, how far into it, how wide a house
is. **Derived fields belong to the chart, not to the renderer** — they are
computed once in `src/derive.rs` and ride on `ChartData`, because the PDF, the
artifact and the desktop wheel each used to derive them separately and drifted.

Concretely: a `Body` carries `sign`/`deg`/`min`, and a chart carries
`houseSweeps`. `derive::fill` is their only writer — the compute stage calls it
for a fresh chart, `load_chart` for one reloaded from disk. It recomputes rather
than trusting, so a chart saved before the fields existed and a hand-edited one
that claims a position its longitude contradicts both come out the same.

The counterpart rule: anything specific to one rendering — arc construction,
type sizes, colour — is *not* a derived field and stays with its renderer. The
geometry the renditions share is not a derived field either, but it is not
theirs to each restate: see **Plate**.

Two client cases cannot read the fields and derive through
`desktop/src/lib/derive.ts`, the one deliberate mirror of `src/derive.rs`
(pinned to the same cases by `derive.test.ts`): the **live scrub**, whose
longitudes are tweened between previews so the backend's fields describe the
target rather than the frame on screen, and **house cusps**, which are
longitudes the chart carries rather than bodies and so have no derived position
of their own.

## Plate

The engraved wheel's geometry — where its rings sit, how its graduations are
classed, when crowded bodies step inward. Stated once in `src/plate.rs` and read
by all three renditions: the PDF directly, the artifact through a `/*__PLATE__*/`
substitution beside the chart, the desktop through the generated
`plate.ts`.

Distinct from a **derived field**, which belongs to a *chart*. A plate value
belongs to the *drawing* and is the same for every chart.

A rendition may **depart** from the plate — the orrery is deliberately richer
than paper, so its house labels clear a core medallion the others do not draw.
A departure is an override written next to its reason; everything not overridden
cannot drift. What stays with the renderer regardless: arc construction (the PDF
builds cubics where SVG writes an `A`), type sizes (points on paper against CSS
pixels), and colour.

## Calculation

A house system, a zodiac, and — when the zodiac is sidereal — an ayanamsa.
Chosen as wire codes (`whole-sign`, `tropical`, `lahiri`), resolved to types by
`chart::systems::resolve` under a three-tier rule: **what was asked for**, else
**what is preferred**, else `systems::DEFAULTS`.

One ladder, one home for the three default codes. There were four ladders in two
languages — the build command's, the preview command's with no preference tier,
and two on the frontend gated on different conditions — and the codes appeared
64 times across the core, the CLI, the command layer and four components. The
webview reads them from `calculation_defaults` rather than restating them.

An unknown code is **refused**, not defaulted. It used to become Whole Sign in
silence, so a typo, a stale preference and a real choice were the same input:
the chart came back claiming `whole-sign`, the reading view's selector snapped
to match, and nothing was said.
