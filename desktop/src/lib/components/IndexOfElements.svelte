<script lang="ts">
  import type { ChartData } from "$lib/types";
  import { catOf, degInSign, planetById, relatedTo, signAt, textGlyph } from "$lib/types";
  import { app, focusedTag, peek, selected, toggle, unpeek } from "$lib/state.svelte";

  let { chart }: { chart: ChartData } = $props();

  // The legend cross-lights with the wheel: the focused element (a pin locks
  // it, else the hovered one) and everything it relates to are marked here too,
  // and hovering a row lights the orrery back.
  const focusTag = $derived(focusedTag());
  const related = $derived(focusTag ? relatedTo(chart, focusTag) : new Set<string>());

  const planetName = (id: string) => planetById(chart, id)?.name ?? id;

  // House entries show the ordinal without the trailing "house" word; the
  // suffix to strip is language-specific (empty locale = strip nothing).
  const HOUSE_SUFFIX: Record<string, string> = { en: " House", ru: " дом" };
  const houseSuffix = $derived(HOUSE_SUFFIX[chart.meta.locale ?? "en"] ?? "");

  // Relevance rule (canonical prose lives in templates/reading.html beside
  // syncRelevance; keep the two in step): visible = occupied ∪ selected ∪
  // expanded — occupancy means a body stands in the sign/house, and a
  // selected filter must never hide itself.
  const occupied = $derived(
    new Set(chart.planets.flatMap((p) => [signAt(chart, p.lon).id, `house:${p.house}`])),
  );
  let expanded = $state<Record<string, boolean>>({ signs: false, houses: false });

  interface Entry {
    tag: string;
    glyph: string;
    name: string;
    detail: string;
  }
  interface Column {
    head: string;
    entries: Entry[];
    filterable?: boolean;
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
    {
      head: "signs",
      filterable: true,
      entries: chart.signs.map((s) => ({ tag: s.id, glyph: s.glyph, name: s.name, detail: "" })),
    },
    {
      head: "houses",
      filterable: true,
      entries: chart.houses.map((h) => ({
        tag: h.id,
        glyph: h.label,
        name: h.name.replace(houseSuffix, ""),
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

<details class="index-fold" bind:open={app.indexOpen}>
  <summary class="rubric index-summary">
    <span class="head-group">
      <span class="lbl">Index of Elements</span>
      <span class="caret" aria-hidden="true">
        <svg width="11" height="7" viewBox="0 0 11 7"><path d="M1 1.2 L5.5 5.5 L10 1.2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </span>
    </span>
  </summary>
  <div class="index">
  {#each columns as col (col.head)}
    {@const filtering = col.filterable && !expanded[col.head]}
    {@const shown = filtering
      ? col.entries.filter((e) => occupied.has(e.tag) || selected.has(e.tag))
      : col.entries}
    {@const hidden = col.entries.length - shown.length}
    <div>
      <h3>{col.head}</h3>
      {#each shown as e (e.tag)}
        <button
          class="entry"
          class:focus={focusTag === e.tag}
          class:rel={related.has(e.tag)}
          aria-pressed={selected.has(e.tag)}
          onclick={() => toggle(e.tag)}
          onmouseenter={() => peek(e.tag)}
          onmouseleave={unpeek}
          onfocus={() => peek(e.tag)}
          onblur={unpeek}
        >
          <span class="g g-{catOf(e.tag)}">{textGlyph(e.glyph)}</span>
          <span class="nm">{e.name}</span>
          {#if e.detail}<span class="lead"></span><span class="dt">{e.detail}</span>{/if}
        </button>
      {/each}
      {#if filtering && hidden > 0}
        <button class="more" onclick={() => (expanded[col.head] = true)}>· {hidden} more</button>
      {:else if col.filterable && expanded[col.head]}
        <button class="more" onclick={() => (expanded[col.head] = false)}>· fewer</button>
      {/if}
    </div>
  {/each}
  </div>
</details>

<style>
  /* the whole index folds away to hand the commentary the full panel */
  .index-summary {
    cursor: pointer;
    list-style: none;
  }
  .index-summary::-webkit-details-marker {
    display: none;
  }
  .index-summary:focus-visible {
    outline: 1px dashed var(--hairline);
    outline-offset: 3px;
  }
  .head-group {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
  }
  .index-summary .caret {
    display: inline-flex;
    color: var(--ink-3);
  }
  .index-fold:not([open]) .index-summary {
    margin-bottom: 0.4rem;
  }
  .index-fold:not([open]) .index-summary .caret {
    transform: rotate(-90deg);
  }
  @media (prefers-reduced-motion: no-preference) {
    .index-summary .caret {
      transition: transform var(--dur-fast) var(--ease-out-quint);
    }
  }
  .index {
    display: grid;
    grid-template-columns: 1.05fr 0.95fr 0.9fr 1.45fr;
    gap: 0.4rem 1.6rem;
    margin-bottom: 1.4rem;
  }
  /* the four columns cramp on a narrow panel; fold to a 2×2 plate, then a
     single column, keeping every entry legible. */
  @media (max-width: 1080px) {
    .index {
      grid-template-columns: 1fr 1fr;
      gap: 0.4rem 2rem;
    }
  }
  @media (max-width: 620px) {
    .index {
      grid-template-columns: 1fr;
    }
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
    font-family: var(--font-astro);
  }
  .entry .g.g-house {
    font-family: var(--font-serif);
    font-size: 0.8em;
    letter-spacing: 0.06em;
    width: 2.6em;
    text-align: right;
  }
  .entry .nm {
    border-bottom: 1px solid transparent;
    transition: border-color var(--dur-fast) var(--ease-out-quint);
  }
  .entry:hover .nm,
  .entry.rel .nm {
    border-bottom-color: var(--hairline);
  }
  .entry[aria-pressed="true"] .nm {
    border-bottom-color: var(--ink-2);
  }
  /* cross-lit from the wheel: the focused row brightens, its relations mark */
  .entry {
    transition: background var(--dur-fast) var(--ease-out-quint);
  }
  .entry.focus {
    background: var(--ink-a04);
    color: var(--ink);
  }
  .entry.focus .nm {
    border-bottom-color: var(--ink);
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
  .more {
    display: block;
    padding: 0.12rem 0 0.12rem 2.85em;
    font-size: 0.82rem;
    font-style: italic;
    color: var(--ink-3);
  }
  .more:hover {
    color: var(--ink);
    text-decoration: underline;
  }
</style>
