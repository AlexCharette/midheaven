<script lang="ts">
  // The landing view: a live chart calculator. The engraved wheel is the hero
  // plate, ringed by the date/time instrument limb (TimeRings), standing on the
  // sky with no frame; beside it a column says which sky — the place, then the
  // moment at display size as a turnable dial (MomentDial), then the
  // calculation — and below that the readings worked on last. Every input
  // funnels through `setMoment`; the wheel renders the pose-tweened projection
  // of the newest preview, so the heavens glide — never jump — to the moment.
  import { onMount } from "svelte";
  import { getPreferences, lastPlace, listCharts, setLastPlace } from "$lib/api";
  import { isBusy } from "$lib/busy.svelte";
  import { plural, t } from "$lib/chrome.svelte";
  import { reason } from "$lib/failure";
  import { reopenReading } from "$lib/reopen";
  import { whyCannotOpenAnother } from "$lib/session.svelte";
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
  import { fromMinutes, nowMoment, toMinutes } from "$lib/civil";
  import type { ChartRow, PlaceDto } from "$lib/types";
  import BirthForm from "./BirthForm.svelte";
  import CalcOptions from "./CalcOptions.svelte";
  import Composer from "./Composer.svelte";
  import Journal from "./Journal.svelte";
  import Library from "./Library.svelte";
  import MomentDial from "./MomentDial.svelte";
  import Overlay from "./Overlay.svelte";
  import PlacePicker from "./PlacePicker.svelte";
  import TimeRings from "./TimeRings.svelte";
  import TopBar from "./TopBar.svelte";
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

  const moment = $derived(fromMinutes(draftMinutes()));
  const calcOptions = $derived(options());

  // The chart to render — the target's identity with the pose's angles — is
  // `pose.project`, not this view's business.
  const chart = $derived(displayed());

  function setToNow() {
    const now = nowMoment();
    setMoment(toMinutes(now.date, now.time)!, "now");
  }
  function onPlacePicked(p: PlaceDto) {
    setPlace(p);
    setLastPlace(p.id).catch(() => {});
  }

  // The few readings worked on last, so the most common way into the app —
  // back to someone — is one click from arriving rather than a panel away.
  // A library that cannot be read, or that has no folder yet, simply shows
  // none: the full panel is where that gets explained and fixed.
  const RECENT = 4;
  let recent = $state<ChartRow[]>([]);
  let recentTotal = $state(0);
  let recentErr = $state<string | null>(null);
  onMount(() => {
    listCharts()
      .then((rows) => {
        recent = rows.slice(0, RECENT);
        recentTotal = rows.length;
      })
      .catch(() => {});
  });
  async function openRecent(id: string) {
    recentErr = null;
    try {
      await reopenReading(id);
    } catch (e) {
      recentErr = reason(e);
    }
  }
</script>

<!-- The journal is a screen of its own rather than a sheet over this one: its
     question is asked by pointing at the plate, and pointing wants the room. -->
{#if journalOpen}
  <Journal onclose={() => (journalOpen = false)} />
{:else}
<!-- The app's ways in: what you can go and do reads first, and the one
     action that starts a reading is the one filled control on the screen. -->
<TopBar>
  {#snippet lead()}
    <span class="brand">Midheaven</span>
    <nav class="ways" aria-label="go to">
      <button type="button" onclick={() => (libraryOpen = true)}>Readings</button>
      <button type="button" onclick={() => (journalOpen = true)}>Journal</button>
    </nav>
  {/snippet}
  {#snippet acts()}
    <button type="button" class="btn quiet" onclick={() => (composing = true)} disabled={composing || !chart}>
      <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"
        ><path d="M3 13h2.4L12.6 5.8a1.7 1.7 0 0 0-2.4-2.4L3 10.6V13Z" /><path d="m9.3 4.3 2.4 2.4" /></svg
      >
      Write a note
    </button>
    <button type="button" class="btn primary" onclick={() => (birthOpen = true)}>Cast a natal chart</button>
  {/snippet}
</TopBar>

<!-- Two postures here too, and the second one was already half-built: while
     a note is being written the wheel turns interactive, because clicking it
     is how you say what the note is about. So composing is a working state,
     and the recent readings make way for the composer. -->
<div class="calc-view" data-posture={composing ? "composing" : "surveying"}>
  <!-- The plate on the sky itself: no frame, no outline. The rings and the
       wheel are drawn finely enough to hold the figure on their own. -->
  <figure class="plate">
    <div class="stack">
      <div class="wheel-slot">
        {#if chart}
          <Wheel {chart} interactive={composing} scrubbing={scrubbing()} />
        {/if}
      </div>
      <TimeRings />
    </div>
  </figure>

  <aside class="panel">
    <!-- The sky being shown, said in full: where, then when at display size —
         the moment is the screen's headline, and the controls that set it are
         the words themselves. -->
    <section class="sky" aria-label="the sky shown">
      <div class="place-line">
        <svg class="pin" viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"
          ><path d="M8 14.5s4.6-4.3 4.6-8a4.6 4.6 0 0 0-9.2 0c0 3.7 4.6 8 4.6 8Z" /><circle
            cx="8"
            cy="6.4"
            r="1.6"
          /></svg
        >
        <PlacePicker value={place()} compact ariaLabel="place" onpick={onPlacePicked} />
      </div>

      <MomentDial minutes={draftMinutes()} onchange={setMoment} />

      <div class="calc-line">
        <button type="button" class="btn quiet small now" onclick={setToNow}>
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
            ><circle cx="8" cy="8" r="5.8" /><path d="M8 4.8V8l2.2 1.4" /></svg
          >
          Now
        </button>
        <CalcOptions
          variant="control"
          houseSystem={calcOptions.houseSystem}
          zodiac={calcOptions.zodiac}
          ayanamsa={calcOptions.ayanamsa}
          onchange={setOptions}
        />
      </div>

      <p class="note" class:err={error() !== null} aria-live="polite">
        {error() ?? warnings().join(" · ")}
      </p>
    </section>

    {#if composing}
      <Composer onclose={() => (composing = false)} />
    {:else if recent.length > 0}
      <section class="recent" aria-labelledby="recent-h">
        <h2 id="recent-h">Recent readings</h2>
        <ul>
          {#each recent as r (r.chart.id)}
            <li>
              <button
                type="button"
                class="row"
                onclick={() => openRecent(r.chart.id)}
                disabled={isBusy() || whyCannotOpenAnother() !== null}
                title={whyCannotOpenAnother() ?? undefined}
              >
                <span class="who">{r.person?.name || "Untitled"}</span>
                <span class="count">{plural(t().notes, r.notes)}</span>
                <span class="sub"
                  >{r.person?.born ?? ""}{r.person?.born && r.chart.place ? " · " : ""}{r.chart.place}</span
                >
              </button>
            </li>
          {/each}
        </ul>
        {#if recentErr}<p class="note err">{recentErr}</p>{/if}
        {#if recentTotal > recent.length}
          <button type="button" class="all" onclick={() => (libraryOpen = true)}>
            All {recentTotal} readings
            <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="M6 3.5 10.5 8 6 12.5" /></svg>
          </button>
        {/if}
      </section>
    {/if}
  </aside>
</div>
{/if}

{#if birthOpen}
  <Overlay variant="panel" label="cast a natal chart" title="Cast a natal chart" onclose={() => (birthOpen = false)}>
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
  <Overlay label="saved readings" title="Saved readings" onclose={() => (libraryOpen = false)}>
    <Library onclose={() => (libraryOpen = false)} />
  </Overlay>
{/if}

<style>
  /* The one engraved word left in the chrome: the wordmark keeps the serif and
     the small capitals, at a tracking that reads as a name rather than a
     banner. */
  .brand {
    font-family: var(--font-serif);
    font-size: 1.05rem;
    font-variant: small-caps;
    letter-spacing: 0.12em;
    color: var(--ink);
    white-space: nowrap;
  }
  .ways {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .ways button {
    padding: 0.4rem 0.7rem;
    border-radius: var(--radius-sm);
    font-size: 0.86rem;
    font-weight: 450;
    color: var(--ink-2);
    text-decoration: none;
    transition:
      color var(--dur-fast) var(--ease-out-quint),
      background var(--dur-fast) var(--ease-out-quint);
  }
  .ways button:hover {
    color: var(--ink);
    background: var(--surface-hover);
  }
  /* --- the plate and its column --- */
  .calc-view {
    min-height: calc(100vh - var(--bar-h));
    display: grid;
    grid-template-columns: auto minmax(17rem, 22rem);
    align-items: center;
    justify-content: center;
    gap: clamp(1.6rem, 4.5vw, 4rem);
    padding: 0 clamp(1.2rem, 3vw, 2rem) clamp(1.2rem, 3vh, 2rem);
  }
  .plate {
    margin: 0;
    width: min(calc(100vh - 110px), 58vw, 800px);
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
  .panel {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2.2rem;
  }

  /* --- the sky --- */
  .place-line {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    margin-bottom: 0.9rem;
    font-family: var(--font-sans);
    font-size: 0.88rem;
    color: var(--ink-2);
  }
  .pin {
    flex: none;
    fill: none;
    stroke: var(--brass);
    stroke-width: 1.2;
  }
  .place-line :global(.picker) {
    flex: 1;
    min-width: 0;
  }
  .place-line :global(input) {
    font-family: var(--font-sans);
    font-variant: normal;
    letter-spacing: 0;
    color: var(--ink-2);
    border-bottom-color: transparent;
    padding: 0.2rem 0;
    text-overflow: ellipsis;
    transition:
      border-color var(--dur-fast) var(--ease-out-quint),
      color var(--dur-fast) var(--ease-out-quint);
  }
  .place-line :global(input:hover),
  .place-line :global(input:focus) {
    color: var(--ink);
    border-bottom-color: var(--hairline);
  }
  .calc-line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    margin-top: 1.2rem;
  }
  /* reserved lines so DST notes never shift the column */
  .note {
    min-height: 1.4em;
    margin: 0.7rem 0 0;
    font-family: var(--font-sans);
    font-size: 0.78rem;
    color: var(--ink-3);
    max-width: 24rem;
  }
  .note.err {
    color: var(--oxblood);
  }

  /* --- recent readings --- */
  .recent h2 {
    margin: 0 0 0.5rem;
    font-family: var(--font-sans);
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--ink-3);
  }
  .recent ul {
    list-style: none;
    margin: 0 -0.6rem;
    padding: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    column-gap: 1rem;
    width: 100%;
    padding: 0.55rem 0.6rem;
    border-radius: var(--radius-sm);
    text-align: left;
    transition: background var(--dur-fast) var(--ease-out-quint);
  }
  .row:hover:not(:disabled) {
    background: var(--surface-hover);
  }
  .row:disabled {
    cursor: wait;
  }
  /* the person is the one line in the list set in the reading's own voice */
  .who {
    font-family: var(--font-serif);
    font-size: 0.98rem;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    align-self: center;
    font-family: var(--font-sans);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
  }
  .sub {
    grid-column: 1 / -1;
    font-family: var(--font-sans);
    font-size: 0.78rem;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .all {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    margin-top: 0.45rem;
    font-family: var(--font-sans);
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--brass);
  }
  .all svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform var(--dur-fast) var(--ease-out-quint);
  }
  .all:hover svg {
    transform: translateX(2px);
  }

  /* --- narrow windows: the column returns beneath the plate --- */
  @media (max-width: 940px) {
    .calc-view {
      grid-template-columns: 1fr;
      justify-items: center;
      align-items: start;
      gap: 1.6rem;
    }
    .plate {
      width: min(78vh, 92vw, 620px);
    }
    .panel {
      width: min(92vw, 26rem);
    }
  }
  @media (max-width: 640px) {
    .ways {
      margin-left: -0.7rem;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ways button,
    .row,
    .all svg {
      transition: none;
    }
  }
</style>
