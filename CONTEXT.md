# Domain language

The words this project uses for its own concepts. Where code and prose have
drifted apart, the entry says which is which — the aim is one word per concept.

Seeded while introducing `src/derive.rs`; extend it rather than coining synonyms.

## Reading

A birth-chart consultation: the words spoken or written about a chart, and the
chart they are about. Not a stored thing — there is no reading record. A
reading is a **chart** plus the **notes** that name it, and "open a reading"
means open that chart and read back its notes.

It used to be a folder: `{slug}_{date}/` holding a `chart.json` and its
transcripts, listed by walking the library. That made a note's home a path
that moves, and made "every note about this person" unanswerable without
opening every folder.

## Chart

Two things wear this name, deliberately, because they are the same sky said
twice.

`contract::ChartData` is the **computed** chart: positions, houses, aspects,
the five aspect kinds, and the localized names for all of them. It is derived,
never stored, and it carries no notes.

`store::Chart` is the **record**: a `BirthSeed` plus the calculation codes —
everything needed to recompute the first — and the person whose sky it is. It
has an id, and that id is what a note names. Ten notes about one transit share
one record rather than ten copies of one sky.

A chart's birth moment is part of its identity. Change it and you have a
different sky, so you get a new record; notes stay with the one they were
written about. A **person's** name is not: correct it and every chart of
theirs reads the correction, because none of them copied it.

## Person

Someone a chart is of — `store::Person`. Separate from the chart because one
person has several: a natal chart, the transits over it, a progression.

## Note

One piece of writing filed under the chart elements it refers to. The type is
`contract::Note`. Say *note* everywhere — prose, UI, comments, code. It replaced
two words for one thing: the type was `Excerpt`, the UI and the PDF said
"passage", and the doc that told you which to use where was this entry.

Some notes were spoken into a recording and some were typed by hand. The app
does not distinguish them: both file under the same vocabulary, filter through
the same rules, and render alike in the app, the PDF and the artifact. A note
may carry no tags at all, in which case it shows whenever no filter is active.

A note spoken in a recording carries `at`, the "HH:MM:SS" it was said at.
Nothing else marks where a note came from.

The type used to carry a `span` as well — byte offsets into the transcript,
proving the text was a verbatim slice of it. Provenance is still established,
by the **Verify gate**, at the moment of routing; it is simply no longer
carried by every note for the rest of its life, because nothing ever read one
back. Removing it took a sentinel with it: an absent span had been spelled
`[0, 0]` on the wire, which meant an empty span had to be an error, which put
a rule about an impossible case into two separate validators.

A note is a record of its own — `store::StoredNote`, one file per note, named
by its id. It names the chart it is about; nothing contains it. That is what
makes "every note I have written about this planet" a question with an answer,
and it is the single change this vocabulary turns on.

## Tag id

An element identifier in the closed form `{category}:{slug}` — `planet:sun`,
`sign:leo`, `house:5`, `aspect:sun-moon`, `kind:trine`. Language-neutral, so a
reading's language never changes its tags. The five categories are fixed.

`aspect:` names a *pair of bodies* — "this note is about the Sun and the Moon".
`kind:` names the relation — "this note is about a trine". They are separate
because only the second can be said without naming anyone, and the first has to
mean something even in a sky where those two bodies are nowhere near each other.

## Vocabulary

Every tag id that exists, in `vocab::ALL`: eleven planets, twelve signs, twelve
houses, all forty-five pairs of the ten bodies, and the five aspect kinds.
Global and closed — a router may only emit tags from it, and the Verify gate
rejects anything outside it.

It used to be derived per chart, from the elements that chart contained
(`ChartData::vocab`). That made a note untaggable without a chart to tag it
against, which is the thing that kept this app from being a journal.

## Verify gate

The provenance check between a router's output and the notes it becomes: the
span must be a real, non-empty slice of the transcript, and every tag must be
in the vocabulary. Rejections surface as warnings rather than failing the
build.

This is the only place provenance is enforced, and the only place a span
exists. The gate hands each accepted note back *beside* its span rather than
carrying one: `coalesce` needs the spans to take a verbatim slice across a
merge, and `route::file` drops them on the way to storage.

## View

A saved question — `query::View`: a name and a `Query`. A question, not an
answer: reopening one runs it again, so a view written last month shows what
was written since.

`Query` is closed on purpose. Five dimensions (tags with any/all, person,
chart, date range, free text), combining with AND, and an empty query is every
note — so a view is always a refinement of "all my notes" and cannot be
written to mean nothing.

## Store

Where everything lives — `store::Store`, four sibling folders of one-file
records under the folder the astrologer chose: `notes/`, `charts/`,
`people/`, `views/`.

One file per record is load-bearing: writing is create-and-amend rather than a
read-modify-write of a shared log, and a file that will not parse costs one
record rather than the folder. Every id is checked before it becomes a
filename, because the webview hands back ids it was given and `../../x` is a
filename too.

## Artifact

The single self-contained offline HTML file a reading exports to. Self-contained
is load-bearing: no external references at all, so it opens from `file://`
forever. Distinct from the PDF, which is a separate rendering of the same chart.

## Session

The open working state: the chart the app currently has open, and possibly a
live recorder capturing a take into it. Commands that need one refuse through
a single guard with a single message (`session::NO_READING`, "no chart has
been built yet").

A session holds no notes. It is the open *window*, and a note outlives the
window it was typed in — so the session holds the chart's id, and the notes
are read from the store.

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

One recording into a live session. Takes accumulate — each is transcribed and
routed on its own, and its notes are filed under the open chart.

"Appended" used to be a choice: routing could replace the chart's note list or
add to it, and adding was what let earlier curation survive a second take.
Notes are their own records now, so a take can only add; there is nothing left
to overwrite.

A take is **in flight** from the moment recording starts until its words land in
the chart, which spans two phases: **recording**, while the capture runs, and
**transcribing**, after it stops. The distinction is load-bearing — transcription
takes real time, and a second take begun in that window would be handed the same
session offset as the one still landing, so both takes' folio anchors would claim
the same stretch of the recording. A take that fails to transcribe is
**abandoned**: the reading stays open and never advances the session clock.

## Reproject

Recompute a chart's geometry for a different house system or zodiac. Only the
geometry changes — the record keeps its id, so the notes filed against it stay
filed against it, and the vocabulary is global, so their tags stay valid.

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
64 times across the core, the CLI, the command layer and four components.

The webview holds **no copy of the codes**: it reads them from
`calculation_defaults`, and until that answers its own are **blank**. A blank
code means *not stated*, which is what `Codes::new` already does with one — so
the first frame asks for the backend's defaults by saying nothing, rather than by
naming them and hoping the two agree. What a selector *shows* for an unstated
code is the served default, resolved at the read.

An unknown code is **refused**, not defaulted — house system, ayanamsa and
zodiac alike. Each used to become the historical default in silence, so a typo, a
stale preference and a real choice were the same input: the chart came back
claiming `whole-sign`, the reading view's selector snapped to match, and nothing
was said.

## Plate title

How a chart's plate is headed, shared by the PDF and the artifact — one string,
one wording, both renditions. They used to word it differently ("The Nativity
of" against "The Birth Chart of") because each lived in a separate file.

Its **shape** is part of it, not only its words. English takes a possessive, so
the holder's name sits inside the headline and the plate has one line: *Mira
Holt's birth chart*. Russian cannot — a possessive there needs the name declined
into the genitive, which no format string can do — so it keeps a line above the
name and lets the name stand undeclined beneath: *Натальная карта* / *Мира Холт*.

`i18n::PlateTitle` is therefore a choice (`Inline` or `Above`) rather than a
string, and `render(name)` hands a rendition its headline plus the name to set
beneath, or `None` when the headline already carries it.
