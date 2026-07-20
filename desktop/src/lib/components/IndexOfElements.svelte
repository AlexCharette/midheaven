<script lang="ts">
  import type { ChartData } from "$lib/types";
  import { catOf, degInSign, planetById, signAt, textGlyph } from "$lib/types";
  import { selected, toggle } from "$lib/state.svelte";

  let { chart }: { chart: ChartData } = $props();

  const planetName = (id: string) => planetById(chart, id)?.name ?? id;

  interface Entry {
    tag: string;
    glyph: string;
    name: string;
    detail: string;
  }
  interface Column {
    head: string;
    entries: Entry[];
  }
  const columns: Column[] = $derived([
    {
      head: "planets",
      entries: chart.planets.map((p) => ({
        tag: p.id,
        glyph: p.glyph,
        name: p.name,
        detail: `${degInSign(p.lon)}° ${textGlyph(signAt(chart, p.lon).glyph)}`,
      })),
    },
    { head: "signs", entries: chart.signs.map((s) => ({ tag: s.id, glyph: s.glyph, name: s.name, detail: "" })) },
    {
      head: "houses",
      entries: chart.houses.map((h) => ({
        tag: h.id,
        glyph: h.label,
        name: h.name.replace(" House", ""),
        detail: "",
      })),
    },
    {
      head: "aspects",
      entries: chart.aspects.map((a) => ({
        tag: a.id,
        glyph: a.glyph,
        name: `${planetName(a.a)} – ${planetName(a.b)}`,
        detail: "",
      })),
    },
  ]);
</script>

<h2 class="rubric">Index of Elements</h2>
<div class="index">
  {#each columns as col (col.head)}
    <div>
      <h3>{col.head}</h3>
      {#each col.entries as e (e.tag)}
        <button class="entry" aria-pressed={selected.has(e.tag)} onclick={() => toggle(e.tag)}>
          <span class="g g-{catOf(e.tag)}">{textGlyph(e.glyph)}</span>
          <span class="nm">{e.name}</span>
          {#if e.detail}<span class="lead"></span><span class="dt">{e.detail}</span>{/if}
        </button>
      {/each}
    </div>
  {/each}
</div>

<style>
  .index {
    display: grid;
    grid-template-columns: 1.05fr 0.95fr 0.9fr 1.45fr;
    gap: 0.4rem 1.6rem;
    margin-bottom: 1.4rem;
  }
  h3 {
    font-weight: 400;
    font-style: italic;
    font-size: 0.98rem;
    color: var(--ink-3);
    margin: 0 0 0.35rem;
    padding-bottom: 0.25rem;
    border-bottom: 1px solid var(--line);
  }
  .entry {
    position: relative;
    display: flex;
    align-items: baseline;
    gap: 0.45em;
    width: 100%;
    text-align: left;
    padding: 0.12rem 0 0.12rem 1.15em;
    font-size: 0.92rem;
    color: var(--ink);
  }
  .entry::before {
    content: "☞\FE0E";
    position: absolute;
    left: 0;
    top: 0.12rem;
    color: var(--ink-2);
    opacity: 0;
    font-size: 0.9em;
  }
  .entry[aria-pressed="true"]::before {
    opacity: 1;
  }
  .entry .g {
    flex: none;
    width: 1.25em;
    text-align: center;
  }
  .entry .g.g-house {
    font-size: 0.8em;
    letter-spacing: 0.06em;
    width: 2.6em;
    text-align: right;
  }
  .entry .nm {
    border-bottom: 1px solid transparent;
  }
  .entry:hover .nm {
    border-bottom-color: var(--hairline);
  }
  .entry[aria-pressed="true"] .nm {
    border-bottom-color: var(--ink-2);
  }
  .entry .lead {
    flex: 1;
    border-bottom: 1px dotted var(--line);
    transform: translateY(-0.28em);
    min-width: 0.6em;
  }
  .entry .dt {
    flex: none;
    color: var(--ink-3);
    font-size: 0.85em;
    font-variant-numeric: tabular-nums;
  }
</style>
