<script lang="ts">
  import type { ChartData } from "$lib/types";
  import { catOf, degInSign, planetById, signAt, textGlyph } from "$lib/types";
  import { app, ayanamsas, excerptsMatching, focusedTag, houseSystems, notify } from "$lib/state.svelte";
  import { reproject } from "$lib/api";
  import { swapDuration } from "$lib/motion";
  import { fade } from "svelte/transition";

  let { chart }: { chart: ChartData } = $props();

  const coreSwap = { duration: swapDuration() };

  // Live calculation controls. They mirror the chart's current codes and
  // re-sync whenever the chart changes (a reproject or a reopen), so the line
  // always reflects the active calculation.
  let houseSystem = $state("whole-sign");
  let zodiac = $state("tropical");
  let ayanamsa = $state("lahiri");
  let reprojecting = $state(false);
  $effect(() => {
    houseSystem = chart.meta.house_system || "whole-sign";
    zodiac = chart.meta.ayanamsa ? "sidereal" : "tropical";
    ayanamsa = chart.meta.ayanamsa ?? "lahiri";
  });

  async function recalc() {
    reprojecting = true;
    try {
      app.chart = await reproject(houseSystem, zodiac, zodiac === "sidereal" ? ayanamsa : null);
    } catch (e) {
      notify(`${e}`, "error");
      // Snap the selects back to what the (unchanged) chart actually is.
      houseSystem = chart.meta.house_system || "whole-sign";
      zodiac = chart.meta.ayanamsa ? "sidereal" : "tropical";
      ayanamsa = chart.meta.ayanamsa ?? "lahiri";
    } finally {
      reprojecting = false;
    }
  }

  // The hub reads out the focused element (a pin locks it, else the hovered
  // one); with nothing focused the centre stays clear and only the corner
  // title cartouche shows.
  const focusTag = $derived(focusedTag());
  const cat = $derived(focusTag ? catOf(focusTag) : "");
  const count = $derived(
    focusTag ? excerptsMatching(chart, [focusTag], "any").length : 0,
  );
  const passages = (n: number) => `${n} ${n === 1 ? "passage" : "passages"}`;

  const planetName = (id: string) => planetById(chart, id)?.name ?? id;
  const planetGlyph = (id: string) => planetById(chart, id)?.glyph ?? "";
  const roman = (n: number) => chart.houses[n - 1]?.label ?? String(n);

  // The orb (deviation from the exact aspect angle) is computed once in the
  // backend and carried on the aspect — the read-out just formats it, rather
  // than re-deriving it from longitudes with a duplicated angle table.
  const fmtOrb = (orb: number) => `${orb < 1 ? orb.toFixed(1) : Math.round(orb)}° orb`;

  // planets standing in a sign / tenanting a house — the read-out's "occupants"
  const planetsInSign = (signId: string) =>
    chart.planets.filter((p) => signAt(chart, p.lon).id === signId);
  const planetsInHouse = (n: number) => chart.planets.filter((p) => p.house === n);
</script>

<!-- the plate's title cartouche, tucked in the corner like an atlas figure -->
<div class="plate-caption">
  <p class="who">{chart.meta.name}</p>
  <p class="vitals">{chart.meta.born}</p>
  <p class="vitals">{chart.meta.place}</p>
  <span class="double-rule" aria-hidden="true"></span>
  {#if chart.meta.birth}
    <div class="calc">
      <select class="calc-sel" aria-label="house system" bind:value={houseSystem} onchange={recalc} disabled={reprojecting}>
        {#each houseSystems as h (h.code)}<option value={h.code}>{h.label}</option>{/each}
      </select>
      <span class="sep" aria-hidden="true">·</span>
      <select class="calc-sel" aria-label="zodiac" bind:value={zodiac} onchange={recalc} disabled={reprojecting}>
        <option value="tropical">Tropical</option>
        <option value="sidereal">Sidereal</option>
      </select>
      {#if zodiac === "sidereal"}
        <select class="calc-sel" aria-label="ayanamsa" bind:value={ayanamsa} onchange={recalc} disabled={reprojecting}>
          {#each ayanamsas as a (a.code)}<option value={a.code}>{a.label}</option>{/each}
        </select>
      {/if}
    </div>
  {:else}
    <p class="system">{chart.meta.system} · {chart.meta.zodiac}</p>
  {/if}
</div>

<!-- the hub read-out appears only while an element is focused; at rest the
     centre stays clear so the aspect web reads uninterrupted -->
{#if focusTag}
  <div class="core">
    {#key focusTag}
      <div class="core-body" in:fade={coreSwap}>
      {#if cat === "planet"}
        {@const p = planetById(chart, focusTag)}
        {#if p}
          {@const s = signAt(chart, p.lon)}
          {@const aspects = chart.aspects.filter((a) => a.a === focusTag || a.b === focusTag)}
          <span class="glyph g-planet">{textGlyph(p.glyph)}</span>
          <p class="name">{p.name}</p>
          <p class="pos">
            {degInSign(p.lon)}° <span class="astro g-sign">{textGlyph(s.glyph)}</span> {s.name}
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
          <p class="count">{passages(count)}</p>
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
          <p class="count">{passages(count)}</p>
        {/if}
      {:else if cat === "house"}
        {@const h = chart.houses.find((x) => x.id === focusTag)}
        {#if h}
          {@const n = Number(focusTag.split(":")[1])}
          {@const cusp = chart.houseCusps[n - 1]}
          {@const cs = signAt(chart, cusp)}
          {@const occ = planetsInHouse(n)}
          <span class="glyph roman g-house">{h.label}</span>
          <p class="name">{h.name}</p>
          <p class="pos sub">cusp {degInSign(cusp)}° <span class="astro g-sign">{textGlyph(cs.glyph)}</span></p>
          <p class="occ">
            {#if occ.length}
              {#each occ as p (p.id)}<span class="astro g-planet" title={p.name}>{textGlyph(p.glyph)}</span>{/each}
            {:else}<span class="empty">no bodies here</span>{/if}
          </p>
          <p class="count">{passages(count)}</p>
        {/if}
      {:else if cat === "aspect"}
        {@const a = chart.aspects.find((x) => x.id === focusTag)}
        {#if a}
          <span class="glyph g-aspect">{textGlyph(a.glyph)}</span>
          <p class="name">{planetName(a.a)} – {planetName(a.b)}</p>
          <p class="pos sub nature-{a.nature}">{a.name} · {a.nature}</p>
          <p class="pos sub">{fmtOrb(a.orb)}</p>
          <p class="count">{passages(count)}</p>
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
  /* --- title cartouche: chart identity, in the plate's top-left corner --- */
  .plate-caption {
    position: absolute;
    top: 0.7rem;
    left: 0.85rem;
    max-width: 13.5rem;
    z-index: 1;
    pointer-events: none;
  }
  .plate-caption .who {
    font-size: 0.98rem;
    color: var(--ink);
    line-height: 1.25;
    text-wrap: balance;
  }
  .plate-caption .vitals {
    font-size: 0.72rem;
    font-style: italic;
    color: var(--ink-3);
    line-height: 1.4;
  }
  .plate-caption .double-rule {
    display: block;
    width: 3.4rem;
    margin: 0.45rem 0;
  }
  .plate-caption .system {
    font-size: 0.64rem;
    font-variant: small-caps;
    letter-spacing: 0.12em;
    color: var(--ink-2);
  }
  /* the calc line is a live control: re-enable pointer events (the caption is
     a pointer-transparent overlay) and let the small selects wrap in the tight
     cartouche. Styled like the form's minimal selects — engraved, chromeless. */
  .plate-caption .calc {
    pointer-events: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.1rem 0.3rem;
    font-size: 0.64rem;
    font-variant: small-caps;
    letter-spacing: 0.12em;
    color: var(--ink-2);
  }
  .plate-caption .calc-sel {
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--line);
    color: var(--ink-2);
    font: inherit;
    font-variant: small-caps;
    letter-spacing: inherit;
    padding: 0 0.1rem;
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease-out-quint),
      border-color var(--dur-fast) var(--ease-out-quint);
  }
  .plate-caption .calc-sel:hover,
  .plate-caption .calc-sel:focus-visible {
    color: var(--ink);
    border-bottom-color: var(--hairline);
  }
  .plate-caption .calc-sel:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .plate-caption .calc option {
    background: var(--bg-deep);
    color: var(--ink);
    font-variant: normal;
    letter-spacing: normal;
  }
  .plate-caption .calc .sep {
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
  .count {
    margin-top: 0.3rem;
    font-size: 0.68rem;
    font-style: italic;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
</style>
