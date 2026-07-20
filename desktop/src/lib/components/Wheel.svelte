<script lang="ts">
  import type { ChartData } from "$lib/types";
  import { degInSign, norm360, planetById, textGlyph } from "$lib/types";
  import { selected, toggle } from "$lib/state.svelte";

  let { chart }: { chart: ChartData } = $props();

  // Geometry ports templates/reading.html: ASC on the left, ecliptic
  // longitude increasing counterclockwise.
  const CX = 360;
  const CY = 360;
  const R = {
    outer: 348,
    bandOut: 344,
    signIn: 306,
    gradIn: 294,
    planet: 260,
    wedgeOut: 230,
    chord: 222,
    hub: 92,
    houseLbl: 112,
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

  const grads = $derived(
    Array.from({ length: 360 }, (_, d) => {
      const len = d % 10 === 0 ? 12 : d % 5 === 0 ? 8 : 4.5;
      const w = d % 10 === 0 ? 0.9 : d % 5 === 0 ? 0.7 : 0.45;
      const [x1, y1] = pt(d, R.signIn);
      const [x2, y2] = pt(d, R.signIn - len);
      return { x1, y1, x2, y2, w };
    }),
  );

  const hubRays = $derived(
    Array.from({ length: 8 }, (_, k) => {
      const [x1, y1] = pt(k * 45, 5);
      const [x2, y2] = pt(k * 45, k % 2 === 0 ? 22 : 13);
      return { x1, y1, x2, y2 };
    }),
  );

  // element washes (canonical treatment lives in templates/reading.html)
  const ELEMENTS = ["fire", "earth", "air", "water"] as const;
  const signBands = $derived(
    chart.signs.map((s, i) => {
      const lon = i * 30;
      const [gx, gy] = pt(lon + 15, 325);
      return {
        s,
        d: sector(lon, lon + 30, R.signIn, R.bandOut),
        gx,
        gy,
        element: ELEMENTS[i % 4],
      };
    }),
  );

  const houseWedges = $derived(
    chart.houseCusps.map((c, i) => {
      const next = chart.houseCusps[(i + 1) % 12];
      const sweep = norm360(next - c) || 30; // equal cusps mean a full sign
      const [sx1, sy1] = pt(c, R.hub);
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
      const [x1, y1] = pt(lon, R.hub);
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
</script>

<svg viewBox="-28 -28 776 776" role="img" aria-label="Natal chart wheel; the index of elements offers the same filters">
  {#each [R.outer, R.bandOut, R.signIn, R.gradIn, R.wedgeOut, R.hub, R.hub - 4] as r, i (r)}
    <circle cx={CX} cy={CY} {r} class={i < 2 ? "engrave-strong" : "engrave"} />
  {/each}
  {#each grads as g, i (i)}
    <line x1={g.x1} y1={g.y1} x2={g.x2} y2={g.y2} class="grad" stroke-width={g.w} />
  {/each}
  {#each hubRays as ray, i (i)}
    <line x1={ray.x1} y1={ray.y1} x2={ray.x2} y2={ray.y2} class="hub-ray" />
  {/each}
  <circle cx={CX} cy={CY} r="3" class="engrave-strong" />

  {#each signBands as band (band.s.id)}
    <path d={band.d} class="wash wash-{band.element}" />
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <path
      d={band.d}
      class="sign-band"
      class:sel={selected.has(band.s.id)}
      role="button"
      tabindex="-1"
      aria-label="{band.s.name} — {band.element}"
      onclick={() => toggle(band.s.id)}
    ><title>{band.s.name} — {band.element}</title></path>
    <text x={band.gx} y={band.gy} class="sign-glyph" text-anchor="middle" dominant-baseline="central"
      >{textGlyph(band.s.glyph)}</text
    >
  {/each}

  {#each houseWedges as w (w.h.id)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <path
      d={w.d}
      class="house-wedge"
      class:sel={selected.has(w.h.id)}
      role="button"
      tabindex="-1"
      aria-label={w.h.name}
      onclick={() => toggle(w.h.id)}
    ><title>{w.h.name}</title></path>
    <line x1={w.spoke.x1} y1={w.spoke.y1} x2={w.spoke.x2} y2={w.spoke.y2} class="cusp-spoke" />
    <text x={w.lx} y={w.ly} class="house-label" text-anchor="middle" dominant-baseline="central">{w.h.label}</text>
  {/each}

  {#each axes as ax (ax.label)}
    <line x1={ax.x1} y1={ax.y1} x2={ax.x2} y2={ax.y2} class="axis" />
    <text x={ax.tx} y={ax.ty} class="axis-label" text-anchor="middle" dominant-baseline="central">{ax.label}</text>
  {/each}

  {#each chords as c (c.a.id)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <g class="aspect" class:sel={selected.has(c.a.id)} role="button" tabindex="-1" onclick={() => toggle(c.a.id)}>
      <line x1={c.x1} y1={c.y1} x2={c.x2} y2={c.y2} class="chord" />
      <line x1={c.x1} y1={c.y1} x2={c.x2} y2={c.y2} class="chord-hit" />
      <title>{c.a.name} {textGlyph(c.a.glyph)}</title>
    </g>
  {/each}

  {#each planets as pl (pl.p.id)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <g class="planet" class:sel={selected.has(pl.p.id)} role="button" tabindex="-1" onclick={() => toggle(pl.p.id)}>
      <line x1={pl.tick.x1} y1={pl.tick.y1} x2={pl.tick.x2} y2={pl.tick.y2} class="tick" />
      <circle cx={pl.gx} cy={pl.gy} r="15" class="halo" />
      <text x={pl.gx} y={pl.gy} class="glyph" font-size={pl.p.glyph.length > 1 ? 13 : 22} text-anchor="middle" dominant-baseline="central"
        >{textGlyph(pl.p.glyph)}</text
      >
      <text x={pl.dx} y={pl.dy} class="deg" text-anchor="middle" dominant-baseline="central">{pl.deg}°</text>
      <title>{pl.p.name}</title>
    </g>
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
  /* element washes — hand-tinted plates; whisper, never shout */
  .wash {
    pointer-events: none;
  }
  .wash-fire {
    fill: rgba(196, 87, 58, 0.1);
  }
  .wash-earth {
    fill: rgba(164, 130, 61, 0.14);
  }
  .wash-air {
    fill: rgba(196, 183, 121, 0.12);
  }
  .wash-water {
    fill: rgba(95, 143, 201, 0.1);
  }
  .hub-ray {
    stroke: var(--line);
  }
  .sign-band {
    fill: transparent;
    stroke: var(--line);
    cursor: pointer;
  }
  .sign-band:hover {
    fill: rgba(53, 171, 124, 0.1);
  }
  .sign-band.sel {
    fill: rgba(53, 171, 124, 0.22);
    stroke: var(--verdigris);
  }
  .sign-glyph {
    fill: var(--verdigris);
    font-size: 21px;
    pointer-events: none;
  }
  .house-wedge {
    fill: transparent;
    cursor: pointer;
  }
  .house-wedge:hover {
    fill: rgba(123, 160, 224, 0.08);
  }
  .house-wedge.sel {
    fill: rgba(123, 160, 224, 0.18);
  }
  .house-label {
    fill: var(--steel);
    font-size: 13px;
    letter-spacing: 0.08em;
    pointer-events: none;
  }
  .cusp-spoke {
    stroke: var(--line);
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
  .planet {
    cursor: pointer;
  }
  .planet .glyph {
    fill: var(--brass);
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
  }
  .planet:hover .halo {
    stroke: rgba(196, 154, 48, 0.5);
  }
  .planet.sel .halo {
    fill: rgba(196, 154, 48, 0.16);
    stroke: var(--brass);
  }
  .chord {
    stroke: var(--oxblood);
    stroke-width: 1.4;
    opacity: 0.6;
  }
  .chord-hit {
    stroke: transparent;
    stroke-width: 11;
    cursor: pointer;
  }
  .aspect:hover .chord {
    opacity: 1;
    stroke-width: 2.2;
  }
  .aspect.sel .chord {
    opacity: 1;
    stroke-width: 3;
  }
</style>
