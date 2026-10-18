<script lang="ts">
  // The landing view: a live chart calculator. The engraved wheel is the hero
  // plate, ringed by the date/time instrument limb (TimeRings); beneath it a
  // figcaption cradle carries the place, the moment fields, the year switch,
  // and the calculation selectors. Every input funnels through `setMoment`;
  // the wheel renders the pose-tweened projection of the newest preview, so
  // the heavens glide — never jump — to the chosen moment.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { expoOut } from "svelte/easing";
  import { swapDuration } from "$lib/motion";
  import { getPreferences, lastPlace, setLastPlace } from "$lib/api";
  import {
    displayed,
    error,
    isSeeded,
    options,
    place,
    refresh,
    scrubbing,
    seed,
    setMoment,
    setOptions,
    setPlace,
    minutes as draftMinutes,
    warnings,
  } from "$lib/preview.svelte";
  import { MONTH_NAMES, fromMinutes, nowMoment, parseDate, setYear, toMinutes } from "$lib/civil";
  import { ayanamsas, houseSystems } from "$lib/options.svelte";
  import { SIDEREAL } from "$lib/types";
  import type { PlaceDto } from "$lib/types";
  import BirthForm from "./BirthForm.svelte";
  import CalcOptions from "./CalcOptions.svelte";
  import Composer from "./Composer.svelte";
  import Journal from "./Journal.svelte";
  import Library from "./Library.svelte";
  import MomentField from "./MomentField.svelte";
  import Overlay from "./Overlay.svelte";
  import PlacePicker from "./PlacePicker.svelte";
  import TimeRings from "./TimeRings.svelte";
  import Wheel from "./Wheel.svelte";

  // Fly-outs this view owns; nobody else's business, so local rather than
  // shared state. `birthOpen` used to sit in the preview pipeline's object,
  // beside its in-flight and error fields. Preferences was here too, which is
  // why it was unreachable from a reading — it belongs to the app, and the
  // layout now owns it and its corner control.
  let libraryOpen = $state(false);
  let journalOpen = $state(false);
  let birthOpen = $state(false);
  // Composing is the one state in which the calculator's plate is an index
  // rather than a display: `Wheel`'s own comment says `interactive` off means
  // "a display of a moment rather than an index of a reading", and while a note
  // is being written that is exactly what it stops being. Pins are cleared on
  // the way in and out by the composer, so nothing a previous note was filed
  // under can leak into the next one or leave the plate lit.
  let composing = $state(false);

  onMount(async () => {
    // A first run seeds the moment and the calculation choices from
    // preferences; a return visit finds the explored moment intact and only
    // refreshes the language.
    const firstRun = !isSeeded();
    if (!place()) {
      try {
        seed({ place: await lastPlace() });
      } catch {
        /* no gazetteer place — the picker is still there */
      }
    }
    try {
      const p = await getPreferences();
      seed({
        ...(p.default_locale ? { lang: p.default_locale } : {}),
        ...(firstRun && p.default_house_system ? { houseSystem: p.default_house_system } : {}),
        ...(firstRun && p.default_zodiac ? { zodiac: p.default_zodiac } : {}),
        ...(firstRun && p.default_ayanamsa ? { ayanamsa: p.default_ayanamsa } : {}),
      });
    } catch {
      /* defaults stand */
    }
    if (firstRun) {
      const now = nowMoment();
      seed({ minutes: toMinutes(now.date, now.time)! });
    }
    // One request for the whole seeded draft.
    refresh("now");
  });

  // The sky's controls fold away, because once the moment is where you want
  // it they are apparatus and the plate is the point. What they fold *to*
  // matters more than the fold: a collapsed summary still has to say which
  // sky this is, or the wheel above it becomes a picture of nothing.
  let momentOpen = $state(true);

  const moment = $derived(fromMinutes(draftMinutes()));
  const year = $derived(Number(moment.date.slice(0, 4)));
  const calcOptions = $derived(options());

  /** The folded line: place, moment, calculation — everything the open
   * controls say, in the order they say it. */
  const summary = $derived.by(() => {
    const d = parseDate(moment.date);
    const when = d
      ? `${d.d} ${MONTH_NAMES[d.m - 1].slice(0, 3)} ${d.y}, ${moment.time}`
      : moment.time;
    const house =
      houseSystems.find((h) => h.code === calcOptions.houseSystem)?.label ??
      calcOptions.houseSystem;
    const zodiac =
      calcOptions.zodiac === SIDEREAL
        ? (ayanamsas.find((a) => a.code === calcOptions.ayanamsa)?.label ?? "Sidereal")
        : "Tropical";
    return { where: place()?.label ?? "", when, calc: `${house} · ${zodiac}` };
  });

  // The chart to render — the target's identity with the pose's angles — is
  // `pose.project`, not this view's business.
  const chart = $derived(displayed());

  function commitDate(text: string) {
    const min = toMinutes(text, moment.time);
    if (min !== null) setMoment(min, "field");
  }
  function commitTime(text: string) {
    const min = toMinutes(moment.date, text);
    if (min !== null) setMoment(min, "field");
  }
  function stepYear(delta: number) {
    const min = toMinutes(setYear(moment.date, year + delta), moment.time);
    if (min !== null) setMoment(min, "stepper");
  }
  function setToNow() {
    const now = nowMoment();
    setMoment(toMinutes(now.date, now.time)!, "now");
  }
  function onPlacePicked(p: PlaceDto) {
    setPlace(p);
    setLastPlace(p.id).catch(() => {});
  }
</script>

<!-- Two postures here too, and the second one was already half-built: while
     a note is being written the wheel turns interactive, because clicking it
     is how you say what the note is about. So composing is a working state,
     and the title page gets out of its way. -->
<div class="calc-view" data-posture={composing ? "composing" : "surveying"}>
  <figure class="plate">
    <div class="plate-frame">
      <div class="stack">
        <div class="wheel-slot">
          {#if chart}
            <Wheel {chart} interactive={composing} scrubbing={scrubbing()} />
          {/if}
        </div>
        <TimeRings />
      </div>
    </div>
  </figure>

  <!-- the margin panel: title cartouche and the moment's apparatus, set
       beside the plate like an atlas figure's legend column -->
  <aside class="panel">
    <header class="masthead">
      <h1>MIDHEAVEN</h1>
      <div class="double-rule"></div>
      <p class="apparatus-text tagline">your offline astrology workbench</p>
    </header>

    <button
      type="button"
      class="sky-summary"
      class:open={momentOpen}
      aria-expanded={momentOpen}
      aria-controls="sky-body"
      onclick={() => (momentOpen = !momentOpen)}
    >
      <span class="over-lbl">the heavens over</span>
      <span class="caret" aria-hidden="true">
        <svg width="11" height="7" viewBox="0 0 11 7"
          ><path
            d="M1 1.2 L5.5 5.5 L10 1.2"
            fill="none"
            stroke="currentColor"
            stroke-width="1.2"
            stroke-linecap="round"
            stroke-linejoin="round"
          /></svg
        >
      </span>
    </button>

    {#if momentOpen}
      <div id="sky-body" transition:slide={{ duration: swapDuration(), easing: expoOut }}>
        <div class="place-line">
          <PlacePicker value={place()} compact ariaLabel="place" onpick={onPlacePicked} />
        </div>

        <div class="moment">
      <label class="lbl" for="calc-date">on</label>
      <MomentField id="calc-date" type="date" value={moment.date} width="10.5rem" oncommit={commitDate} />
      <label class="lbl" for="calc-time">at</label>
      <MomentField id="calc-time" type="time" value={moment.time} width="8rem" oncommit={commitTime} />
      <span class="lbl">year</span>
      <span class="year-switch">
        <button type="button" class="ghost chev" onclick={() => stepYear(-1)} aria-label="previous year">‹</button>
        <span class="year">{year}</span>
        <button type="button" class="ghost chev" onclick={() => stepYear(1)} aria-label="next year">›</button>
      </span>
          <span class="lbl" aria-hidden="true"></span>
          <button type="button" class="ghost now" onclick={setToNow}>set to now</button>
        </div>

        <div class="calc-line">
          <CalcOptions
            variant="caption"
            houseSystem={calcOptions.houseSystem}
            zodiac={calcOptions.zodiac}
            ayanamsa={calcOptions.ayanamsa}
            onchange={setOptions}
          />
        </div>
      </div>
    {:else}
      <!-- folded, the sky still has to name itself: the wheel above is a
           picture of a moment, and this is the caption that says which -->
      <dl class="folded" transition:slide={{ duration: swapDuration(), easing: expoOut }}>
        <dt class="sr-only">place</dt>
        <dd class="where">{summary.where}</dd>
        <dt class="sr-only">moment</dt>
        <dd class="when">{summary.when}</dd>
        <dt class="sr-only">calculation</dt>
        <dd class="calc">{summary.calc}</dd>
      </dl>
    {/if}
    <p class="note" class:err={error() !== null} aria-live="polite">
      {error() ?? warnings().join(" · ")}
    </p>

    {#if composing}
      <Composer onclose={() => (composing = false)} />
    {/if}
  </aside>
</div>

<footer>
  <button type="button" class="ghost" onclick={() => (libraryOpen = true)}>open a saved reading</button>
  <button type="button" class="ghost" onclick={() => (composing = true)} disabled={composing || !chart}>
    write a note
  </button>
  <button type="button" class="ghost" onclick={() => (journalOpen = true)}>the journal</button>
  <button type="button" class="frame-btn cast" onclick={() => (birthOpen = true)}>
    cast a natal chart
  </button>
</footer>

{#if birthOpen}
  <Overlay variant="panel" label="cast a natal chart" onclose={() => (birthOpen = false)}>
    <BirthForm
      initial={{
        date: moment.date,
        time: moment.time,
        place: place(),
        houseSystem: calcOptions.houseSystem,
        zodiac: calcOptions.zodiac,
        ayanamsa: calcOptions.ayanamsa,
      }}
      onclose={() => (birthOpen = false)}
    />
  </Overlay>
{:else if libraryOpen}
  <Overlay label="saved readings" onclose={() => (libraryOpen = false)}>
    <Library onclose={() => (libraryOpen = false)} />
  </Overlay>
{:else if journalOpen}
  <Overlay label="the journal" onclose={() => (journalOpen = false)}>
    <Journal onclose={() => (journalOpen = false)} />
  </Overlay>
{/if}

<style>
  /* the plate takes the viewport; the apparatus sits in a margin column
     beside it, like the legend of an atlas figure */
  .calc-view {
    min-height: 100vh;
    display: grid;
    grid-template-columns: auto minmax(16rem, 21rem);
    /* The legend hangs from the plate's top rule, not from its midpoint.
       Centred, it floated with equal dead space above and below and aligned
       to nothing; an atlas sets its key at the head of the margin and lets
       the rest of the margin be margin. */
    align-items: start;
    justify-content: center;
    gap: clamp(1.4rem, 4vw, 3.2rem);
    padding: clamp(0.5rem, 1.5vh, 1.1rem) 1.4rem 3.4rem;
  }
  /* the plate's frame insets it 6px, so the masthead drops to meet the rule
     rather than the figure's bounding box */
  .panel {
    padding-top: calc(6px + 0.55rem);
  }
  .masthead {
    margin-bottom: clamp(1rem, 3.5vh, 2.2rem);
  }
  /* A title page is for arriving, not for working. Once the plate is being
     used as an index it is the one thing on screen with nothing to say. */
  .calc-view[data-posture="composing"] .masthead {
    opacity: 0.32;
  }
  @media (prefers-reduced-motion: no-preference) {
    .masthead {
      transition: opacity var(--dur-slow) var(--ease-out-expo);
    }
  }
  h1 {
    font-weight: 400;
    font-size: clamp(1.15rem, 2.4vw, 1.45rem);
    letter-spacing: 0.34em;
    margin: 0;
  }
  .masthead .double-rule {
    width: 4.5rem;
    margin: 0.55rem 0;
  }
  .tagline {
    margin: 0;
    font-size: 0.78rem;
  }
  .plate {
    margin: 0;
    width: min(86vh, 58vw, 780px);
  }
  .plate-frame {
    position: relative;
    border: 1px solid var(--hairline);
    outline: 1px solid var(--line);
    outline-offset: 5px;
    padding: 0.55rem;
    margin: 6px;
    background: radial-gradient(ellipse at 50% 42%, var(--plate-glow) 0%, transparent 70%);
  }
  /* the rings SVG (960 units) and the wheel (824) stack on one grid cell;
     scaling the wheel by 824/960 makes their user units identical, so the
     rings sit exactly outside the drift ring with no runtime measuring */
  .stack {
    display: grid;
  }
  .stack > :global(*) {
    grid-area: 1 / 1;
  }
  .wheel-slot {
    width: calc(100% * 824 / 960);
    place-self: center;
  }
  /* --- the legend column --- */
  .panel {
    min-width: 0;
  }
  /* The fold's head, in the same grammar the element index uses: a small-caps
     line and a caret that lies down when shut. */
  .sky-summary {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    padding: 0;
    margin: 0 0 0.35rem;
    font-size: 0.78rem;
    font-variant: small-caps;
    letter-spacing: 0.14em;
    color: var(--ink-3);
    text-align: left;
  }
  .sky-summary:hover {
    color: var(--ink-2);
  }
  .sky-summary .caret {
    display: inline-flex;
  }
  .sky-summary:not(.open) .caret {
    transform: rotate(-90deg);
  }
  @media (prefers-reduced-motion: no-preference) {
    .sky-summary .caret,
    .sky-summary {
      transition:
        transform var(--dur-fast) var(--ease-out-quint),
        color var(--dur-fast) var(--ease-out-quint);
    }
  }
  .place-line {
    font-size: 0.92rem;
    margin-bottom: 1.1rem;
  }
  /* Folded: the caption the plate needs, set as a short ledger. The place
     keeps the weight it has open; the moment and the calculation step down,
     so the line reads in the order you set them. */
  .folded {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .folded dd {
    margin: 0;
  }
  .folded .where {
    font-size: 0.92rem;
    color: var(--ink);
  }
  .folded .when {
    font-size: 0.86rem;
    color: var(--ink-2);
    font-variant-numeric: tabular-nums;
  }
  .folded .calc {
    font-size: 0.78rem;
    font-style: italic;
    color: var(--ink-3);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  /* the moment reads as ledger rows: italic labels in a slim gutter, one
     value per line — nothing shares a line it could be squeezed off of */
  .moment {
    display: grid;
    grid-template-columns: 3.2rem auto;
    justify-content: start;
    gap: 0.5rem 0.9rem;
    align-items: baseline;
  }
  .lbl {
    font-style: italic;
    color: var(--ink-3);
    font-size: 0.9rem;
    text-align: right;
  }
  .year-switch {
    display: inline-flex;
    align-items: baseline;
    gap: 0.35rem;
  }
  .year {
    font-variant-numeric: tabular-nums;
    color: var(--ink);
    min-width: 3ch;
    text-align: center;
  }
  .chev {
    font-size: 1rem;
    padding: 0 0.25rem;
  }
  .now {
    font-size: 0.85rem;
    justify-self: start;
  }
  .calc-line {
    margin-top: 1.1rem;
    font-size: 0.72rem;
  }
  /* reserved lines so DST notes never shift the column */
  .note {
    min-height: 2.6em;
    margin: 0.6rem 0 0;
    font-size: 0.78rem;
    font-style: italic;
    color: var(--ink-3);
    max-width: 24rem;
  }
  .note.err {
    color: var(--oxblood);
  }
  /* Four acts do not fit one narrow line, and letting them wrap turned the
     stamp into three stacked words sitting on top of the calculation line.
     Below this width the rail is two rows — the quiet acts, then the stamp —
     and the page reserves the height for it. */
  @media (max-width: 560px) {
    footer {
      flex-wrap: wrap;
      gap: 0.5rem 1.1rem;
      padding-bottom: 0.6rem;
    }
    footer .cast {
      margin-left: 0;
      flex-basis: 100%;
      text-align: center;
      padding: 0.35rem 0;
      white-space: nowrap;
    }
    .calc-view {
      padding-bottom: 6.4rem;
    }
  }
  /* narrow windows: the legend returns beneath the plate */
  @media (max-width: 940px) {
    .calc-view {
      grid-template-columns: 1fr;
      justify-items: center;
      gap: 1.4rem;
    }
    .plate {
      width: min(78vh, 92vw, 620px);
    }
    .panel {
      width: min(92vw, 26rem);
    }
  }
  /* --- footer: quiet library/prefs acts left, the one brass act right --- */
  /* the same console rail the reading view carries: one hairline, the
     page's width, the acts set on it as marks with a single stamp */
  footer {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: var(--z-footer);
    display: flex;
    align-items: center;
    gap: 1.1rem;
    padding: 0.5rem clamp(1.2rem, 3vw, 2.6rem);
    background: var(--bg-deep);
    border-top: 1px solid var(--line);
    font-size: 0.88rem;
  }
  footer .cast {
    margin-left: auto;
    padding: 0.4rem 1.6rem;
    letter-spacing: 0.18em;
    border-color: var(--brass);
    color: var(--ink);
    transition: background var(--dur-base) var(--ease-out-quint);
  }
  /* The hover was a zero-offset brass ring — a halo, which is decoration
     rather than depth, and this world engraves instead of glowing. The wash
     alone says it. */
  footer .cast:hover {
    background: var(--brass-wash);
  }
  /* the title settles in over the plate's self-draw */
  @media (prefers-reduced-motion: no-preference) {
    h1,
    .tagline {
      opacity: 0;
      animation: settle 0.7s var(--ease-out-quint) forwards;
    }
    h1 {
      animation-delay: 0.18s;
    }
    .tagline {
      animation-delay: 0.3s;
    }
  }
  @keyframes settle {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
