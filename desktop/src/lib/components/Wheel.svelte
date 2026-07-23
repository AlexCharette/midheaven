<script lang="ts">
  import type { ChartData } from "$lib/types";
  import {
    catOf, degInSign, norm360, planetById, relatedTo, signDensity, textGlyph,
  } from "$lib/types";
  import { focusedTag, peek, selected, toggle, unpeek } from "$lib/state.svelte";
  import type { Snippet } from "svelte";

  let { chart }: { chart: ChartData } = $props();

  // Every element on the plate is a live index: hover or keyboard-focus to
  // preview and light up its relations, click/Enter to pin it. A pin locks the
  // focus (hover stops flipping it); otherwise the hovered tag drives the
  // illumination and the selector pointer.
  const focusTag = $derived(focusedTag());
  const related = $derived(focusTag ? relatedTo(chart, focusTag) : new Set<string>());
  const pinKey = (tag: string) => (e: KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      toggle(tag);
      e.preventDefault();
    }
  };

  // Geometry ports templates/reading.html: ASC on the left, ecliptic
  // longitude increasing counterclockwise. Radii grow outward from the hub;
  // the outer two tracks (density, drift) and the enlarged hub are new to the
  // orrery.
  const CX = 360;
  const CY = 360;
  const R = {
    drift: 398, // decorative idle-drift ring (outermost)
    passOut: 382, // passage-density bars grow toward here
    passIn: 356, // density baseline ring
    outer: 348,
    bandOut: 344,
    signIn: 306,
    gradIn: 294,
    planet: 260,
    wedgeOut: 230,
    chord: 222,
    houseLbl: 170, // sits in the house band, outside the focus-only core medallion
    hub: 92, // inner edge of the house-wedge band + the hub ring
    hubInner: 8, // cusp spokes and axes converge here, at the compass centre
  };

  const asc = $derived(chart.axes.asc);

  function pt(lon: number, r: number): [number, number] {
    const a = Math.PI + ((lon - asc) * Math.PI) / 180;
    return [CX + r * Math.cos(a), CY - r * Math.sin(a)];
  }
  function sector(l1: number, l2: number, r1: number, r2: number): string {
    const [x1, y1] = pt(l1, r2);
    const [x2, y2] = pt(l2, r2);
    const [x3, y3] = pt(l2, r1);
    const [x4, y4] = pt(l1, r1);
    const large = norm360(l2 - l1) > 180 ? 1 : 0;
    return `M${x1} ${y1} A${r2} ${r2} 0 ${large} 0 ${x2} ${y2} L${x3} ${y3} A${r1} ${r1} 0 ${large} 1 ${x4} ${y4} Z`;
  }

  // Degree graduations: 1°/5°/10° tick hierarchy, decade ticks a touch longer
  // so the ring reads as a proper scale.
  const grads = $derived(
    Array.from({ length: 360 }, (_, d) => {
      const len = d % 10 === 0 ? 14 : d % 5 === 0 ? 8 : 4.5;
      const w = d % 10 === 0 ? 0.9 : d % 5 === 0 ? 0.7 : 0.45;
      const [x1, y1] = pt(d, R.signIn);
      const [x2, y2] = pt(d, R.signIn - len);
      return { x1, y1, x2, y2, w };
    }),
  );

  // A ring of faint star marks that drifts slowly — the only moving,
  // non-data ring. Positions are arbitrary (the whole ring rotates).
  const driftStars = $derived(
    Array.from({ length: 12 }, (_, k) => {
      const [x, y] = pt(k * 30, R.drift);
      return { x, y };
    }),
  );

  // Passage-density track — how much the reading dwelt on each sign.
  const density = $derived(signDensity(chart));
  const maxDensity = $derived(Math.max(1, ...density));

  // element washes (canonical treatment lives in templates/reading.html);
  // the element itself comes from the contract (sign Ref.element)
  const signBands = $derived(
    chart.signs.map((s, i) => {
      const lon = i * 30;
      const [gx, gy] = pt(lon + 15, 325);
      const w = density[i];
      const len = w > 0 ? Math.sqrt(w / maxDensity) * (R.passOut - R.passIn) : 0;
      const bar = w > 0 ? sector(lon + 5, lon + 25, R.passIn, R.passIn + len) : "";
      return { s, d: sector(lon, lon + 30, R.signIn, R.bandOut), gx, gy, bar, w };
    }),
  );

  const houseWedges = $derived(
    chart.houseCusps.map((c, i) => {
      const next = chart.houseCusps[(i + 1) % 12];
      const sweep = norm360(next - c) || 30; // equal cusps mean a full sign
      const [sx1, sy1] = pt(c, R.hubInner); // spokes converge at the centre
      const [sx2, sy2] = pt(c, R.gradIn);
      const [lx, ly] = pt(c + sweep / 2, R.houseLbl);
      return {
        h: chart.houses[i],
        d: sector(c, c + sweep, R.hub, R.wedgeOut),
        spoke: { x1: sx1, y1: sy1, x2: sx2, y2: sy2 },
        lx,
        ly,
      };
    }),
  );

  const axes = $derived(
    [
      { lon: chart.axes.asc, label: "AC" },
      { lon: chart.axes.mc, label: "MC" },
      { lon: chart.axes.asc + 180, label: "DC" },
      { lon: chart.axes.mc + 180, label: "IC" },
    ].map(({ lon, label }) => {
      const [x1, y1] = pt(lon, R.hubInner); // the AC–DC / MC–IC cross meets at centre
      const [x2, y2] = pt(lon, R.outer);
      const [tx, ty] = pt(lon, R.outer + 13);
      return { label, x1, y1, x2, y2, tx, ty };
    }),
  );

  const lonOf = (id: string) => planetById(chart, id)?.lon ?? 0;
  const chords = $derived(
    chart.aspects.map((a) => {
      const [x1, y1] = pt(lonOf(a.a), R.chord);
      const [x2, y2] = pt(lonOf(a.b), R.chord);
      return { a, x1, y1, x2, y2 };
    }),
  );

  const planets = $derived.by(() => {
    const byLon = [...chart.planets].sort((a, b) => a.lon - b.lon);
    let prevLon: number | null = null;
    let prevR = R.planet;
    return byLon.map((p) => {
      const gap = prevLon === null ? 999 : Math.min(Math.abs(p.lon - prevLon), 360 - Math.abs(p.lon - prevLon));
      const r = gap < 8 ? Math.max(prevR - 27, 176) : R.planet;
      prevLon = p.lon;
      prevR = r;
      const [gx, gy] = pt(p.lon, r);
      const [dx, dy] = pt(p.lon, r - 21);
      const [t1x, t1y] = pt(p.lon, R.gradIn - 8);
      const [t2x, t2y] = pt(p.lon, R.gradIn);
      return { p, gx, gy, dx, dy, tick: { x1: t1x, y1: t1y, x2: t2x, y2: t2y }, deg: degInSign(p.lon) };
    });
  });

  // The longitude the selector pointer aims at — the focused element's own
  // position, a sign/house midpoint, or an aspect's angular bisector.
  const focusLon = $derived.by(() => {
    if (!focusTag) return null;
    const c = catOf(focusTag);
    if (c === "planet") return planetById(chart, focusTag)?.lon ?? null;
    if (c === "sign") {
      const i = chart.signs.findIndex((s) => s.id === focusTag);
      return i < 0 ? null : i * 30 + 15;
    }
    if (c === "house") {
      const n = Number(focusTag.split(":")[1]);
      const cusp = chart.houseCusps[n - 1];
      if (cusp === undefined) return null;
      const sweep = norm360(chart.houseCusps[n % 12] - cusp) || 30;
      return cusp + sweep / 2;
    }
    if (c === "aspect") {
      const a = chart.aspects.find((x) => x.id === focusTag);
      if (!a) return null;
      const la = lonOf(a.a);
      const diff = (((lonOf(a.b) - la + 540) % 360) - 180) / 2;
      return la + diff;
    }
    return null;
  });
  // Rotate a north-pointing index clockwise onto the focused longitude.
  const pointerDeg = $derived.by(() => {
    if (focusLon === null) return 0;
    const [x, y] = pt(focusLon, 1);
    return (Math.atan2(x - CX, -(y - CY)) * 180) / Math.PI;
  });
</script>

<svg
  viewBox="-52 -52 824 824"
  role="img"
  class:focusing={focusTag !== null}
  aria-label="Natal chart wheel; the index of elements offers the same filters"
>
  <!-- every clickable element on the plate shares one interactive contract:
       pin on click/Enter, preview on hover/focus, and the sel/focus/rel
       lighting. Defined once here (in SVG scope) and rendered per shape. -->
  {#snippet sector(id: string, cls: string, label: string | undefined, style: string | undefined, body: Snippet)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <g
      class={cls}
      class:sel={selected.has(id)}
      class:focus={focusTag === id}
      class:rel={related.has(id)}
      role="button"
      tabindex="0"
      aria-label={label}
      {style}
      onclick={() => toggle(id)}
      onmouseenter={() => peek(id)}
      onmouseleave={unpeek}
      onfocus={() => peek(id)}
      onblur={unpeek}
      onkeydown={pinKey(id)}
    >
      {@render body()}
    </g>
  {/snippet}

  <!-- decorative drift ring — the one moving, non-data track -->
  <g class="drift" aria-hidden="true">
    {#each driftStars as s, i (i)}
      <text x={s.x} y={s.y} class="drift-star" text-anchor="middle" dominant-baseline="central">✶</text>
    {/each}
  </g>
  <circle cx={CX} cy={CY} r={R.passIn} class="dens-ring" pathLength="1" style="--d: 0ms" />

  {#each [R.outer, R.bandOut, R.signIn, R.gradIn, R.wedgeOut, R.hub] as r, i (r)}
    <circle cx={CX} cy={CY} {r} pathLength="1" style="--d: {i * 70}ms" class={i < 2 ? "engrave-strong ring" : "engrave ring"} />
  {/each}
  {#each grads as g, i (i)}
    <line x1={g.x1} y1={g.y1} x2={g.x2} y2={g.y2} class="grad" stroke-width={g.w} />
  {/each}
  <!-- the compass centre where the cusp spokes and axes converge -->
  <circle cx={CX} cy={CY} r="2.6" class="engrave-strong hub-dot" />

  {#each signBands as band (band.s.id)}
    {#snippet body()}
      <path d={band.d} class="wash wash-{band.s.element}" />
      <path d={band.d} class="sign-band"><title>{band.s.name} — {band.s.element}</title></path>
      {#if band.bar}
        <path d={band.bar} class="dens-bar"><title>{band.w} {band.w === 1 ? "passage" : "passages"} · {band.s.name}</title></path>
      {/if}
      <text x={band.gx} y={band.gy} class="sign-glyph" text-anchor="middle" dominant-baseline="central"
        >{textGlyph(band.s.glyph)}</text
      >
    {/snippet}
    {@render sector(band.s.id, "sign", `${band.s.name} — ${band.s.element}`, undefined, body)}
  {/each}

  {#each houseWedges as w (w.h.id)}
    {#snippet body()}
      <path d={w.d} class="house-wedge"><title>{w.h.name}</title></path>
      <line x1={w.spoke.x1} y1={w.spoke.y1} x2={w.spoke.x2} y2={w.spoke.y2} class="cusp-spoke" />
      <text x={w.lx} y={w.ly} class="house-label" text-anchor="middle" dominant-baseline="central">{w.h.label}</text>
    {/snippet}
    {@render sector(w.h.id, "house", w.h.name, undefined, body)}
  {/each}

  {#each axes as ax (ax.label)}
    <line x1={ax.x1} y1={ax.y1} x2={ax.x2} y2={ax.y2} class="axis" />
    <text x={ax.tx} y={ax.ty} class="axis-label" text-anchor="middle" dominant-baseline="central">{ax.label}</text>
  {/each}

  <!-- selector pointer — an armillary index that eases onto the focus -->
  <g class="pointer" class:on={focusLon !== null} style="transform: rotate({pointerDeg}deg); transform-origin: {CX}px {CY}px;" aria-hidden="true">
    <line x1={CX} y1={CY - R.hub - 6} x2={CX} y2={CY - R.gradIn + 10} class="pointer-line" />
    <path d="M{CX} {CY - R.gradIn + 2} l-4 8 l8 0 z" class="pointer-head" />
  </g>

  {#each chords as c (c.a.id)}
    {#snippet body()}
      <line x1={c.x1} y1={c.y1} x2={c.x2} y2={c.y2} class="chord nature-{c.a.nature}" />
      <line x1={c.x1} y1={c.y1} x2={c.x2} y2={c.y2} class="chord-hit" />
      <title>{c.a.name} {textGlyph(c.a.glyph)}</title>
    {/snippet}
    {@render sector(c.a.id, "aspect", undefined, undefined, body)}
  {/each}

  {#each planets as pl, i (pl.p.id)}
    {#snippet body()}
      <line x1={pl.tick.x1} y1={pl.tick.y1} x2={pl.tick.x2} y2={pl.tick.y2} class="tick" />
      <circle cx={pl.gx} cy={pl.gy} r="15" class="halo" />
      <text x={pl.gx} y={pl.gy} class="glyph" font-size={pl.p.glyph.length > 1 ? 13 : 22} text-anchor="middle" dominant-baseline="central"
        >{textGlyph(pl.p.glyph)}</text
      >
      <text x={pl.dx} y={pl.dy} class="deg" text-anchor="middle" dominant-baseline="central">{pl.deg}°</text>
      <title>{pl.p.name}</title>
    {/snippet}
    {@render sector(pl.p.id, "planet", undefined, `--d: ${520 + i * 55}ms`, body)}
  {/each}
</svg>

<style>
  svg {
    width: 100%;
    height: auto;
    display: block;
  }
  text {
    font-family: inherit;
    fill: var(--ink-2);
  }
  .engrave {
    stroke: var(--line);
    fill: none;
  }
  .engrave-strong {
    stroke: var(--hairline);
    fill: none;
  }
  .grad {
    stroke: var(--line);
  }
  /* --- passage-density track --- */
  .dens-ring {
    stroke: var(--line);
    fill: none;
  }
  .dens-bar {
    fill: var(--ink-a12);
    stroke: var(--hairline);
    stroke-width: 0.5;
    pointer-events: auto;
    cursor: pointer;
    transition: fill var(--dur-base) var(--ease-out-quint);
  }
  .sign:hover .dens-bar,
  .sign:focus-visible .dens-bar,
  .sign.focus .dens-bar,
  .sign.sel .dens-bar {
    fill: var(--verdigris-wash);
    stroke: var(--verdigris);
  }
  /* --- drift ring (decorative) --- */
  .drift-star {
    fill: var(--ink-3);
    font-size: 8px;
    opacity: 0.5;
  }
  /* element washes — hand-tinted plates; whisper, never shout */
  .wash {
    pointer-events: none;
  }
  .wash-fire {
    fill: var(--wash-fire);
  }
  .wash-earth {
    fill: var(--wash-earth);
  }
  .wash-air {
    fill: var(--wash-air);
  }
  .wash-water {
    fill: var(--wash-water);
  }
  /* --- signs --- */
  /* keyboard focus is shown by each element's own engraved indicator (wash,
     halo, chord) plus the .focus spotlight — never a stray UA box */
  .sign,
  .house,
  .aspect,
  .planet {
    outline: none;
  }
  .sign {
    cursor: pointer;
  }
  .sign-band {
    fill: transparent;
    stroke: var(--line);
    cursor: pointer;
    transition:
      fill var(--dur-base) var(--ease-out-quint),
      stroke var(--dur-base) var(--ease-out-quint);
  }
  .sign:hover .sign-band,
  .sign:focus-visible .sign-band,
  .sign.focus .sign-band,
  .sign.rel .sign-band {
    fill: var(--verdigris-wash);
  }
  .sign.focus .sign-band,
  .sign.sel .sign-band {
    stroke: var(--verdigris);
  }
  .sign-glyph {
    fill: var(--verdigris);
    font-family: var(--font-astro);
    font-size: 21px;
    pointer-events: none;
  }
  /* --- houses --- */
  .house {
    cursor: pointer;
  }
  .house-wedge {
    fill: transparent;
    transition: fill var(--dur-base) var(--ease-out-quint);
  }
  .house:hover .house-wedge,
  .house:focus-visible .house-wedge,
  .house.focus .house-wedge,
  .house.rel .house-wedge,
  .house.sel .house-wedge {
    fill: var(--steel-wash);
  }
  .house-label {
    fill: var(--steel);
    font-size: 13px;
    letter-spacing: 0.08em;
    pointer-events: none;
    transition: fill var(--dur-base) var(--ease-out-quint);
  }
  .house.focus .house-label {
    fill: var(--ink);
  }
  .cusp-spoke {
    stroke: var(--line);
    pointer-events: none;
  }
  .axis {
    stroke: var(--hairline);
    stroke-width: 1.6;
  }
  .axis-label {
    fill: var(--ink-3);
    font-size: 12px;
    letter-spacing: 0.1em;
  }
  /* --- selector pointer --- */
  .pointer {
    opacity: 0;
    pointer-events: none;
  }
  .pointer.on {
    opacity: 1;
  }
  .pointer-line {
    stroke: var(--brass);
    stroke-width: 1;
    opacity: 0.55;
  }
  .pointer-head {
    fill: var(--brass);
    opacity: 0.7;
  }
  @media (prefers-reduced-motion: no-preference) {
    .pointer {
      transition:
        transform var(--dur-slow) var(--ease-out-quint),
        opacity var(--dur-base) var(--ease-out-quint);
    }
  }
  /* --- planets --- */
  .planet {
    cursor: pointer;
  }
  .planet .glyph {
    fill: var(--brass);
    font-family: var(--font-astro);
    transition: filter var(--dur-base) var(--ease-out-quint);
  }
  .planet .deg {
    fill: var(--ink-3);
    font-size: 9.5px;
  }
  .planet .tick {
    stroke: var(--brass);
    stroke-width: 1.2;
  }
  .planet .halo {
    fill: transparent;
    stroke: transparent;
    transition:
      fill var(--dur-base) var(--ease-out-quint),
      stroke var(--dur-base) var(--ease-out-quint);
  }
  .planet:hover .halo,
  .planet:focus-visible .halo,
  .planet.focus .halo,
  .planet.rel .halo {
    stroke: var(--brass-halo);
  }
  .planet.focus .halo,
  .planet.sel .halo {
    fill: var(--brass-wash);
    stroke: var(--brass);
  }
  /* the pinned/focused body glows: a faint brass bloom, engraved not neon */
  .planet.focus .glyph,
  .planet.sel .glyph {
    filter: drop-shadow(0 0 3px var(--brass-halo));
  }
  /* chords carry the aspect's NATURE (classic blue/red), not the category
     color — mirrors templates/reading.html */
  .chord {
    stroke-width: 1.4;
    opacity: 0.6;
    transition:
      opacity var(--dur-base) var(--ease-out-quint),
      stroke-width var(--dur-base) var(--ease-out-quint);
  }
  .chord.nature-harmonious {
    stroke: var(--steel);
  }
  .chord.nature-challenging {
    stroke: var(--oxblood);
  }
  .chord.nature-neutral {
    stroke: var(--ink-3);
  }
  .chord-hit {
    stroke: transparent;
    stroke-width: 11;
    cursor: pointer;
  }
  .aspect:hover .chord,
  .aspect:focus-visible .chord,
  .aspect.rel .chord {
    opacity: 0.95;
    stroke-width: 2;
  }
  .aspect.focus .chord {
    opacity: 1;
    stroke-width: 2.6;
  }
  .aspect.sel .chord {
    opacity: 1;
    stroke-width: 3;
  }

  /* Spotlight: while one element is focused, everything unrelated (and not
     pinned) recedes so the relations read at a glance. Kept gentle — labels
     stay legible, and keyboard focus still lands on a clear target. */
  .sign,
  .house,
  .aspect,
  .planet {
    transition: opacity var(--dur-base) var(--ease-out-quint);
  }
  svg.focusing .sign:not(.focus):not(.rel):not(.sel),
  svg.focusing .house:not(.focus):not(.rel):not(.sel),
  svg.focusing .aspect:not(.focus):not(.rel):not(.sel),
  svg.focusing .planet:not(.focus):not(.rel):not(.sel) {
    opacity: 0.42;
  }

  /* Entrance choreography — the plate draws itself, then settles inward:
     rings ink in from the hub, the apparatus fades up, planets arrive one by
     one, aspect chords last. Mirrors templates/reading.html. Runs once on the
     wheel's mount (the component persists across view changes). */
  @media (prefers-reduced-motion: no-preference) {
    .ring,
    .dens-ring {
      stroke-dasharray: 1;
      stroke-dashoffset: 1;
      animation: ring-draw 0.9s var(--ease-out-quint) forwards;
      animation-delay: var(--d);
    }
    .grad,
    .wash,
    .sign-band,
    .sign-glyph,
    .dens-bar,
    .house-wedge,
    .house-label,
    .cusp-spoke,
    .axis,
    .axis-label,
    .hub-dot,
    .drift-star {
      opacity: 0;
      animation: fade-in var(--dur-slow) var(--ease-out-quint) 0.45s forwards;
    }
    .drift-star {
      animation-delay: 1.1s;
    }
    .planet {
      opacity: 0;
      animation: rise-in 0.55s var(--ease-out-quint) forwards;
      animation-delay: var(--d);
    }
    .aspect {
      opacity: 0;
      animation: fade-in var(--dur-slow) var(--ease-out-quint) 1.05s forwards;
    }
    /* the drift ring turns slowly, forever — a living orrery */
    .drift {
      transform-box: fill-box;
      transform-origin: center;
      animation: drift-spin var(--dur-orrery) linear infinite;
    }
  }
  /* fade-in must not clobber the spotlight opacity once settled */
  svg.focusing .sign:not(.focus):not(.rel):not(.sel),
  svg.focusing .house:not(.focus):not(.rel):not(.sel),
  svg.focusing .aspect:not(.focus):not(.rel):not(.sel),
  svg.focusing .planet:not(.focus):not(.rel):not(.sel) {
    animation: none;
  }
  @keyframes ring-draw {
    to {
      stroke-dashoffset: 0;
    }
  }
  @keyframes fade-in {
    to {
      opacity: 1;
    }
  }
  @keyframes rise-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @keyframes drift-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
