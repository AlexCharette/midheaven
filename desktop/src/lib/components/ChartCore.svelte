<script lang="ts">
  import type { ChartData, Note } from "$lib/types";
  import { SIDEREAL, calculationOf, catOf, planetById, signOf, textGlyph } from "$lib/types";
  // A cusp is a longitude the chart carries, not a body, so it has no derived
  // position to read — the one case in this file that still derives.
  import { positionOf } from "$lib/derive";
  import { noteCount } from "$lib/focus";
  import { focusedTag } from "$lib/focus.svelte";
  import { defaults } from "$lib/options.svelte";
  import { notify } from "$lib/toasts.svelte";
  import { plural, t } from "$lib/chrome.svelte";
  import { applyRecalculation, whyCannotRecalculate } from "$lib/session.svelte";
  import { during, isBusy } from "$lib/busy.svelte";
  import { reproject } from "$lib/api";
  import CalcOptions from "./CalcOptions.svelte";
  import { swapDuration } from "$lib/motion";
  import { fade } from "svelte/transition";
  import { reason } from "$lib/failure";

  let {
    chart,
    notes,
    bare = false,
  }: {
    chart: ChartData;
    notes: Note[];
    /** The plate alone, its calculation controls put away — how it is shown
     * to a client. The hub read-out stays: it is what pointing at the chart
     * is for. */
    bare?: boolean;
  } = $props();

  const coreSwap = { duration: swapDuration() };

  // What the chart WAS computed with, read from the chart itself — a
  // projection, not a copy. Three `$state` slots used to mirror it, re-synced by
  // an `$effect` that also had to be called by hand on every error path.
  const settled = $derived(calculationOf(chart, defaults()));
  // What the astrologer is ASKING for, or null when the selects simply show the
  // chart. A refusal or a failure drops it; there is nothing to roll back to.
  let asked = $state<{ houseSystem: string; zodiac: string; ayanamsa: string } | null>(null);
  const shown = $derived(asked ?? settled);

  async function recalc(next: { houseSystem: string; zodiac: string; ayanamsa: string }) {
    asked = next;
    // `during` puts the reproject on the shared busy phase, so the record
    // button is disabled for its duration rather than only looking safe.
    try {
      const chart = await during("compute", () =>
        reproject(next.houseSystem, next.zodiac, next.zodiac === SIDEREAL ? next.ayanamsa : null),
      );
      // Refused at the write, not only when the control was clicked: a take can
      // begin while the round trip is out, and the machine is what knows.
      applyRecalculation(chart);
    } catch (e) {
      notify(reason(e), "error");
    } finally {
      asked = null;
    }
  }

  // The hub reads out the focused element (a pin locks it, else the hovered
  // one); with nothing focused the centre stays clear and only the corner
  // title cartouche shows.
  const focusTag = $derived(focusedTag());
  const cat = $derived(focusTag ? catOf(focusTag) : "");
  const count = $derived(
    focusTag ? noteCount(notes, focusTag) : 0,
  );
  const notesWord = (n: number) => plural(t().notes, n);

  const planetName = (id: string) => planetById(chart, id)?.name ?? id;
  const planetGlyph = (id: string) => planetById(chart, id)?.glyph ?? "";
  const roman = (n: number) => chart.houses[n - 1]?.label ?? String(n);

  // The orb (deviation from the exact aspect angle) is computed once in the
  // backend and carried on the aspect — the read-out just formats it, rather
  // than re-deriving it from longitudes with a duplicated angle table.
  const fmtOrb = (orb: number) => `${orb < 1 ? orb.toFixed(1) : Math.round(orb)}° orb`;

  // planets standing in a sign / tenanting a house — the read-out's "occupants"
  const planetsInSign = (signId: string) =>
    chart.planets.filter((p) => signOf(chart, p).id === signId);
  const planetsInHouse = (n: number) => chart.planets.filter((p) => p.house === n);
</script>

<!-- The calculation, set in the plate's empty top-left corner where it can be
     changed in place. Who and when moved to the head of the screen: said in
     the corner as well, they were said twice. -->
<div class="plate-caption" class:away={bare} inert={bare}>
  {#if chart.meta.birth}
    <div class="calc-line" title={whyCannotRecalculate() ?? undefined}>
      <CalcOptions
        variant="control"
        houseSystem={shown.houseSystem}
        zodiac={shown.zodiac}
        ayanamsa={shown.ayanamsa}
        disabled={isBusy() || whyCannotRecalculate() !== null}
        onchange={recalc}
      />
    </div>
  {:else}
    <p class="system">{chart.meta.system} · {chart.meta.zodiac}</p>
  {/if}
</div>

<!-- the hub read-out appears only while an element is focused; at rest the
     centre stays clear so the aspect web reads uninterrupted -->
{#if focusTag}
  <div class="core" class:bare>
    {#key focusTag}
      <div class="core-body" in:fade={coreSwap}>
      {#if cat === "planet"}
        {@const p = planetById(chart, focusTag)}
        {#if p}
          {@const s = signOf(chart, p)}
          {@const aspects = chart.aspects.filter((a) => a.a === focusTag || a.b === focusTag)}
          <span class="glyph g-planet">{textGlyph(p.glyph)}</span>
          <p class="name">{p.name}</p>
          <p class="pos">
            {p.deg}° <span class="astro g-sign">{textGlyph(s.glyph)}</span> {s.name}
          </p>
          <p class="pos sub">House {roman(p.house)}</p>
          {#if aspects.length}
            <p class="rel-row">
              {#each aspects as a (a.id)}
                <span class="rel-aspect" title={a.name}>
                  <span class="astro g-aspect">{textGlyph(a.glyph)}</span><span
                    class="astro g-planet">{textGlyph(planetGlyph(a.a === focusTag ? a.b : a.a))}</span>
                </span>
              {/each}
            </p>
          {/if}
          <p class="count">{notesWord(count)}</p>
        {/if}
      {:else if cat === "sign"}
        {@const s = chart.signs.find((x) => x.id === focusTag)}
        {#if s}
          {@const occ = planetsInSign(s.id)}
          <span class="glyph g-sign">{textGlyph(s.glyph)}</span>
          <p class="name">{s.name}</p>
          <p class="pos sub">{s.element}</p>
          <p class="occ">
            {#if occ.length}
              {#each occ as p (p.id)}<span class="astro g-planet" title={p.name}>{textGlyph(p.glyph)}</span>{/each}
            {:else}<span class="empty">no bodies here</span>{/if}
          </p>
          <p class="count">{notesWord(count)}</p>
        {/if}
      {:else if cat === "house"}
        {@const h = chart.houses.find((x) => x.id === focusTag)}
        {#if h}
          {@const n = Number(focusTag.split(":")[1])}
          {@const cuspAt = positionOf(chart.houseCusps[n - 1])}
          {@const cs = chart.signs[cuspAt.sign]}
          {@const occ = planetsInHouse(n)}
          <span class="glyph roman g-house">{h.label}</span>
          <p class="name">{h.name}</p>
          <p class="pos sub">cusp {cuspAt.deg}° <span class="astro g-sign">{textGlyph(cs.glyph)}</span></p>
          <p class="occ">
            {#if occ.length}
              {#each occ as p (p.id)}<span class="astro g-planet" title={p.name}>{textGlyph(p.glyph)}</span>{/each}
            {:else}<span class="empty">no bodies here</span>{/if}
          </p>
          <p class="count">{notesWord(count)}</p>
        {/if}
      {:else if cat === "aspect"}
        {@const a = chart.aspects.find((x) => x.id === focusTag)}
        {#if a}
          <span class="glyph g-aspect">{textGlyph(a.glyph)}</span>
          <p class="name">{planetName(a.a)} – {planetName(a.b)}</p>
          <p class="pos sub nature-{a.nature}">{a.name} · {a.nature}</p>
          <p class="pos sub">{fmtOrb(a.orb)}</p>
          <p class="count">{notesWord(count)}</p>
        {/if}
      {/if}
      </div>
    {/key}
  </div>
{/if}

<style>
  /* The orrery's core: a central cartouche masking the aspect web's crossing.
     Sits over the drawn hub, scaling with the plate; its own hairline frame
     echoes the plate-within-a-plate motif. */
  .core {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 37%;
    aspect-ratio: 1;
    border-radius: 50%;
    border: 1px solid var(--hairline);
    background:
      radial-gradient(circle at 50% 42%, var(--plate-glow) 0%, transparent 72%),
      var(--core-veil);
    box-shadow: 0 0 0 4px transparent;
    outline: 1px solid var(--line);
    outline-offset: 4px;
    display: grid;
    place-items: center;
    text-align: center;
    padding: 6%;
    pointer-events: none; /* the wheel beneath stays fully interactive */
    overflow: hidden;
    container-type: inline-size; /* so the dossier's cqw type scales with the plate */
  }
  .core-body {
    grid-area: 1 / 1; /* stack keyed swaps without reflow */
    max-width: 100%;
  }
  p {
    margin: 0;
  }
  /* --- the calculation, in the plate's corner --- */
  .plate-caption {
    position: absolute;
    top: 0;
    left: 0;
    max-width: 40%;
    z-index: 1;
    transition: opacity 320ms var(--ease-out-quint);
  }
  .plate-caption.away {
    opacity: 0;
    pointer-events: none;
  }
  .plate-caption .system {
    font-family: var(--font-sans);
    font-size: 0.78rem;
    color: var(--ink-3);
  }
  /* --- focused element dossier --- */
  .glyph {
    display: block;
    font-family: var(--font-astro);
    font-size: clamp(1.2rem, 4cqw, 1.9rem);
    line-height: 1;
    margin-bottom: 0.18rem;
  }
  .glyph.roman {
    font-family: var(--font-serif);
    letter-spacing: 0.08em;
  }
  .name {
    font-size: clamp(0.78rem, 2.3cqw, 1rem);
    color: var(--ink);
    line-height: 1.2;
    text-wrap: balance;
  }
  .pos {
    font-size: 0.76rem;
    color: var(--ink-2);
    font-variant-numeric: tabular-nums;
    line-height: 1.4;
  }
  .pos.sub {
    color: var(--ink-3);
  }
  .pos .astro,
  .occ .astro,
  .rel-row .astro {
    font-family: var(--font-astro);
  }
  .occ {
    font-size: 0.9rem;
    line-height: 1.3;
    margin-top: 0.1rem;
    display: flex;
    flex-wrap: wrap;
    gap: 0.15em 0.3em;
    justify-content: center;
  }
  .occ .empty {
    font-family: var(--font-serif);
    font-size: 0.72rem;
    font-style: italic;
    color: var(--ink-3);
  }
  .rel-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.1em 0.45em;
    justify-content: center;
    margin-top: 0.2rem;
    font-size: 0.82rem;
  }
  .rel-aspect {
    display: inline-flex;
    align-items: baseline;
    gap: 0.08em;
  }
  .nature-harmonious {
    color: var(--steel);
  }
  .nature-challenging {
    color: var(--oxblood);
  }
  /* how much the astrologer has written is theirs, not the client's */
  .core.bare .count {
    display: none;
  }
  .count {
    margin-top: 0.3rem;
    font-size: 0.68rem;
    font-style: italic;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
</style>
