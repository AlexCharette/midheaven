<script lang="ts">
  // The landing view: a live chart calculator. The engraved wheel is the hero
  // plate, ringed by the date/time instrument limb (TimeRings); beneath it a
  // figcaption cradle carries the place, the moment fields, the year switch,
  // and the calculation selectors. Every input funnels through `setDraft`;
  // the wheel renders the pose-tweened projection of the newest preview, so
  // the heavens glide — never jump — to the chosen moment.
  import { onMount } from "svelte";
  import { getPreferences, lastPlace, setLastPlace } from "$lib/api";
  import { calc, pose, requestPreview, setDraft } from "$lib/calc.svelte";
  import { fromMinutes, nowMoment, parseDate, parseTime, setYear, toMinutes } from "$lib/civil";
  import type { ChartData } from "$lib/types";
  import { norm360, positionOf, sweepOf } from "$lib/derive";
  import type { PlaceDto } from "$lib/types";
  import BirthForm from "./BirthForm.svelte";
  import CalcOptions from "./CalcOptions.svelte";
  import Library from "./Library.svelte";
  import MomentField from "./MomentField.svelte";
  import Overlay from "./Overlay.svelte";
  import PlacePicker from "./PlacePicker.svelte";
  import Preferences from "./Preferences.svelte";
  import TimeRings from "./TimeRings.svelte";
  import Wheel from "./Wheel.svelte";

  let libraryOpen = $state(false);
  let prefsOpen = $state(false);

  onMount(async () => {
    const firstRun = calc.minutes === 0;
    if (!calc.place) {
      try {
        calc.place = await lastPlace();
      } catch {
        /* no gazetteer place — the picker is still there */
      }
    }
    try {
      const p = await getPreferences();
      if (p.default_locale) calc.lang = p.default_locale;
      if (firstRun) {
        if (p.default_house_system) calc.houseSystem = p.default_house_system;
        if (p.default_zodiac) calc.zodiac = p.default_zodiac;
        if (p.default_ayanamsa) calc.ayanamsa = p.default_ayanamsa;
      }
    } catch {
      /* defaults stand */
    }
    if (firstRun) {
      const now = nowMoment();
      calc.minutes = toMinutes(now.date, now.time)!;
    }
    requestPreview("now");
  });

  const moment = $derived(fromMinutes(calc.minutes));
  const year = $derived(Number(moment.date.slice(0, 4)));

  // The wheel's chart: the target's identity with the pose's angles — planets,
  // frame, and cusps mid-glide; aspects/houses/labels discrete from the target.
  //
  // The derived fields (`sign`/`deg`/`min`, `houseSweeps`) belong to the target's
  // longitudes, so they are re-derived for the tweened ones — this is the live
  // scrub that `$lib/derive` exists for. Everything downstream reads the fields
  // and so needs no knowledge of the glide.
  const displayed = $derived.by((): ChartData | null => {
    const t = calc.target;
    if (!t) return null;
    const cur = pose.current;
    const cusps = t.houseCusps.map((c, i) => norm360(cur.cusps[i] ?? c));
    return {
      ...t,
      axes: { asc: norm360(cur.asc), mc: norm360(cur.mc) },
      houseCusps: cusps,
      houseSweeps: cusps.map((c, i) => sweepOf(c, cusps[(i + 1) % cusps.length])),
      planets: t.planets.map((p) => {
        const lon = norm360(cur.lons[p.id] ?? p.lon);
        return { ...p, lon, ...positionOf(lon) };
      }),
    };
  });

  function commitDate(text: string) {
    const min = toMinutes(text, moment.time);
    if (min !== null) setDraft(min, "field");
  }
  function commitTime(text: string) {
    const min = toMinutes(moment.date, text);
    if (min !== null) setDraft(min, "field");
  }
  function stepYear(delta: number) {
    const min = toMinutes(setYear(moment.date, year + delta), moment.time);
    if (min !== null) setDraft(min, "stepper");
  }
  function setToNow() {
    const now = nowMoment();
    setDraft(toMinutes(now.date, now.time)!, "now");
  }
  function onPlacePicked(p: PlaceDto) {
    setLastPlace(p.id).catch(() => {});
    requestPreview("field");
  }
</script>

<div class="calc-view">
  <figure class="plate">
    <div class="plate-frame">
      <div class="stack">
        <div class="wheel-slot">
          {#if displayed}
            <Wheel chart={displayed} interactive={false} scrubbing={calc.scrubbing} />
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

    <div class="place-line">
      <p class="over-lbl">the heavens over</p>
      <PlacePicker bind:value={calc.place} compact ariaLabel="place" onpick={onPlacePicked} />
    </div>

    <div class="moment">
      <span class="lbl">on</span>
      <MomentField
        value={moment.date}
        placeholder="YYYY-MM-DD"
        label="date"
        parse={parseDate}
        oncommit={commitDate}
      />
      <span class="lbl">at</span>
      <MomentField
        value={moment.time}
        placeholder="HH:MM"
        label="time"
        width="5.5rem"
        parse={parseTime}
        oncommit={commitTime}
      />
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
        bind:houseSystem={calc.houseSystem}
        bind:zodiac={calc.zodiac}
        bind:ayanamsa={calc.ayanamsa}
        onchange={() => requestPreview("field")}
      />
    </div>
    <p class="note" class:err={calc.error !== null} aria-live="polite">
      {calc.error ?? calc.warnings.join(" · ")}
    </p>
  </aside>
</div>

<footer>
  <span class="quiet-acts">
    <button type="button" class="ghost" onclick={() => (libraryOpen = true)}>open a saved reading</button>
    <span class="sep" aria-hidden="true">·</span>
    <button type="button" class="ghost" onclick={() => (prefsOpen = true)}>preferences</button>
  </span>
  <button type="button" class="frame-btn cast" onclick={() => (calc.birthOpen = true)}>
    cast a natal chart
  </button>
</footer>

{#if calc.birthOpen}
  <Overlay variant="panel" label="cast a natal chart" onclose={() => (calc.birthOpen = false)}>
    <BirthForm
      initial={{
        date: moment.date,
        time: moment.time,
        place: calc.place,
        houseSystem: calc.houseSystem,
        zodiac: calc.zodiac,
        ayanamsa: calc.ayanamsa,
      }}
      onclose={() => (calc.birthOpen = false)}
    />
  </Overlay>
{:else if libraryOpen}
  <Overlay label="saved readings" onclose={() => (libraryOpen = false)}>
    <Library onclose={() => (libraryOpen = false)} />
  </Overlay>
{:else if prefsOpen}
  <Overlay label="preferences" onclose={() => (prefsOpen = false)}>
    <Preferences onclose={() => (prefsOpen = false)} />
  </Overlay>
{/if}

<style>
  /* the plate takes the viewport; the apparatus sits in a margin column
     beside it, like the legend of an atlas figure */
  .calc-view {
    min-height: 100vh;
    display: grid;
    grid-template-columns: auto minmax(16rem, 21rem);
    align-items: center;
    justify-content: center;
    gap: clamp(1.4rem, 4vw, 3.2rem);
    padding: clamp(0.5rem, 1.5vh, 1.1rem) 1.4rem 3.4rem;
  }
  .masthead {
    margin-bottom: clamp(1rem, 3.5vh, 2.2rem);
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
  .place-line .over-lbl {
    margin: 0 0 0.35rem;
    font-size: 0.78rem;
    font-variant: small-caps;
    letter-spacing: 0.14em;
    color: var(--ink-3);
  }
  .place-line {
    font-size: 0.92rem;
    margin-bottom: 1.1rem;
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
  footer {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: var(--z-footer);
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.45rem 1.4rem;
    background: var(--bg-deep);
    border-top: 1px solid var(--hairline);
    font-size: 0.88rem;
  }
  footer .quiet-acts .sep {
    color: var(--ink-3);
    margin: 0 0.4rem;
  }
  footer .cast {
    margin-left: auto;
    padding: 0.4rem 1.6rem;
    letter-spacing: 0.18em;
    border-color: var(--brass);
    color: var(--ink);
    transition:
      background var(--dur-base) var(--ease-out-quint),
      box-shadow var(--dur-base) var(--ease-out-quint);
  }
  footer .cast:hover {
    background: var(--brass-wash);
    box-shadow: 0 0 0 1px var(--brass-halo);
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
