<script lang="ts">
  // The vocabulary drawn as a wheel, with no sky behind it.
  //
  // The journal holds notes from every chart, and their tags are global —
  // `planet:saturn` means Saturn in anyone's reading — so the plate it needs
  // is not any one chart. This is the wheel's grammar with the positions taken
  // out: the twelve signs in their band from Aries at the left, twelve equal
  // houses inside them, the bodies set evenly on their own track, and the five
  // aspect kinds at the hub. Nothing on it claims a placement — there are no
  // degrees, no cusps, no chords — and the caption beside it says so.
  //
  // It keeps the real wheel's interactive contract (pin on click/Enter,
  // preview on hover/focus, the shared focus store) and its `data-tag` hook,
  // so the journal can zoom either plate the same way.
  import { textGlyph } from "$lib/types";
  import type { ElementDto } from "$lib/types";
  import { PLATE } from "$lib/generated/plate";
  import { focusedTag, isPinned, peek, toggle, unpeek } from "$lib/focus.svelte";

  let {
    elements,
    counts,
  }: {
    elements: ElementDto[];
    /** Notes filed under each tag — the plate's density. */
    counts: Map<string, number>;
  } = $props();

  const CX = 360;
  const CY = 360;
  const R = PLATE.radii;
  // The bodies' own track, between the house wedges and the graduations, and
  // the ring the aspect kinds sit on inside the hub.
  const BODY = 262;
  const KIND = 52;
  const HOUSE_OUT = R.wedgeOut;
  const HOUSE_IN = R.hub;

  // Ecliptic longitude to plate coordinates, Aries 0° at nine o'clock and
  // running counter-clockwise, the way every chart in the app is drawn.
  const at = (lon: number, r: number) => {
    const a = ((180 + lon) * Math.PI) / 180;
    return { x: CX + r * Math.cos(a), y: CY - r * Math.sin(a) };
  };
  function band(lon0: number, lon1: number, rIn: number, rOut: number): string {
    const a = at(lon0, rOut);
    const b = at(lon1, rOut);
    const c = at(lon1, rIn);
    const d = at(lon0, rIn);
    return `M${a.x} ${a.y} A${rOut} ${rOut} 0 0 0 ${b.x} ${b.y} L${c.x} ${c.y} A${rIn} ${rIn} 0 0 1 ${d.x} ${d.y} Z`;
  }

  const ELEMENT = ["fire", "earth", "air", "water"];
  const of = (cat: string) => elements.filter((e) => e.cat === cat);

  const signs = $derived(
    of("sign").map((e, i) => ({
      e,
      d: band(i * 30, i * 30 + 30, R.signIn, R.bandOut),
      g: at(i * 30 + 15, R.signGlyph),
      element: ELEMENT[i % 4],
    })),
  );
  const houses = $derived(
    of("house").map((e, i) => ({
      e,
      d: band(i * 30, i * 30 + 30, HOUSE_IN, HOUSE_OUT),
      g: at(i * 30 + 15, (HOUSE_IN + HOUSE_OUT) / 2),
    })),
  );
  const bodies = $derived.by(() => {
    const list = of("planet");
    return list.map((e, i) => ({ e, g: at(90 + (i * 360) / list.length, BODY) }));
  });
  const kinds = $derived.by(() => {
    const list = of("kind");
    return list.map((e, i) => ({ e, g: at(90 + (i * 360) / list.length, KIND) }));
  });

  const focusTag = $derived(focusedTag());
  const n = (tag: string) => counts.get(tag) ?? 0;
  const label = (e: ElementDto) => {
    const c = n(e.tag);
    return `${e.name}, ${c} ${c === 1 ? "note" : "notes"}`;
  };
  const key = (tag: string) => (ev: KeyboardEvent) => {
    if (ev.key === "Enter" || ev.key === " ") {
      toggle(tag);
      ev.preventDefault();
    }
  };
</script>

<svg viewBox="-52 -52 824 824" role="group" aria-label="the vocabulary" class:focusing={focusTag !== null}>
  {#snippet el(e: ElementDto, cls: string, body: import("svelte").Snippet)}
    <g
      class="{cls} el"
      class:empty={n(e.tag) === 0}
      class:sel={isPinned(e.tag)}
      class:focus={focusTag === e.tag}
      data-tag={e.tag}
      role="button"
      tabindex="0"
      aria-label={label(e)}
      aria-pressed={isPinned(e.tag)}
      onclick={() => toggle(e.tag)}
      onmouseenter={() => peek(e.tag)}
      onmouseleave={unpeek}
      onfocus={() => peek(e.tag)}
      onblur={unpeek}
      onkeydown={key(e.tag)}
    >
      {@render body()}
    </g>
  {/snippet}

  <circle cx={CX} cy={CY} r={R.outer} class="ring strong" />
  <circle cx={CX} cy={CY} r={R.gradIn} class="ring" />
  <circle cx={CX} cy={CY} r={BODY + 26} class="ring faint" />
  <circle cx={CX} cy={CY} r={BODY - 26} class="ring faint" />

  {#each signs as s (s.e.tag)}
    {#snippet body()}
      <path d={s.d} class="wash wash-{s.element}" />
      <path d={s.d} class="cell" />
      <text x={s.g.x} y={s.g.y} class="glyph sign-glyph astro" text-anchor="middle" dominant-baseline="central"
        >{textGlyph(s.e.glyph)}</text
      >
    {/snippet}
    {@render el(s.e, "sign", body)}
  {/each}

  {#each houses as h (h.e.tag)}
    {#snippet body()}
      <path d={h.d} class="cell house-cell" />
      <text x={h.g.x} y={h.g.y - 6} class="house-label" text-anchor="middle" dominant-baseline="central"
        >{h.e.glyph}</text
      >
      {#if n(h.e.tag) > 0}
        <text x={h.g.x} y={h.g.y + 13} class="count" text-anchor="middle" dominant-baseline="central">{n(h.e.tag)}</text>
      {/if}
    {/snippet}
    {@render el(h.e, "house", body)}
  {/each}

  <circle cx={CX} cy={CY} r={R.hub} class="ring strong" />

  {#each bodies as b (b.e.tag)}
    {#snippet body()}
      <circle cx={b.g.x} cy={b.g.y} r="19" class="halo" />
      <text
        x={b.g.x}
        y={b.g.y}
        class="glyph body-glyph astro"
        font-size={b.e.glyph.length > 1 ? 13 : 23}
        text-anchor="middle"
        dominant-baseline="central">{textGlyph(b.e.glyph)}</text
      >
      {#if n(b.e.tag) > 0}
        <text x={b.g.x + 19} y={b.g.y - 15} class="count" text-anchor="start" dominant-baseline="central"
          >{n(b.e.tag)}</text
        >
      {/if}
    {/snippet}
    {@render el(b.e, "body", body)}
  {/each}

  {#each kinds as k (k.e.tag)}
    {#snippet body()}
      <circle cx={k.g.x} cy={k.g.y} r="14" class="halo" />
      <text x={k.g.x} y={k.g.y} class="glyph kind-glyph astro" text-anchor="middle" dominant-baseline="central"
        >{textGlyph(k.e.glyph)}</text
      >
    {/snippet}
    {@render el(k.e, "kind", body)}
  {/each}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    height: auto;
    overflow: visible;
  }
  .ring {
    fill: none;
    stroke: var(--line);
  }
  .ring.strong {
    stroke: var(--hairline);
  }
  .ring.faint {
    stroke: rgba(233, 228, 211, 0.07);
    stroke-dasharray: 2 5;
  }
  .el {
    cursor: pointer;
    outline: none;
    transition: opacity var(--dur-base) var(--ease-out-quint);
  }
  .cell {
    fill: transparent;
    stroke: var(--line);
    transition: fill var(--dur-fast) var(--ease-out-quint);
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
  .glyph {
    font-family: var(--font-astro);
  }
  .sign-glyph {
    font-size: 22px;
    fill: var(--verdigris);
  }
  .body-glyph {
    fill: var(--brass);
  }
  .kind-glyph {
    font-size: 15px;
    fill: var(--oxblood);
  }
  .house-label {
    font-family: var(--font-serif);
    font-size: 17px;
    fill: var(--steel);
  }
  .count {
    font-family: var(--font-sans);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    fill: var(--ink-3);
  }
  .halo {
    fill: transparent;
    stroke: transparent;
    transition:
      stroke var(--dur-fast) var(--ease-out-quint),
      fill var(--dur-fast) var(--ease-out-quint);
  }
  /* an element nothing has been written about recedes, but stays pickable */
  .el.empty .glyph,
  .el.empty .house-label {
    opacity: 0.45;
  }
  .el:hover .cell,
  .el:focus-visible .cell,
  .el.focus .cell {
    fill: var(--ink-a04);
  }
  .el.sel .cell {
    fill: var(--brass-wash);
    stroke: var(--brass-halo);
  }
  .el:hover .halo,
  .el:focus-visible .halo,
  .el.focus .halo {
    stroke: var(--hairline);
    fill: var(--ink-a04);
  }
  .el.sel .halo {
    stroke: var(--brass);
    fill: var(--brass-wash);
  }
  .el.sel .glyph,
  .el.sel .house-label {
    opacity: 1;
  }
  /* the same spotlight the chart wheel uses: everything not in play steps back */
  svg.focusing .el:not(.focus):not(.sel) {
    opacity: 0.4;
  }
  @media (prefers-reduced-motion: reduce) {
    .el,
    .cell,
    .halo {
      transition: none;
    }
  }
</style>
