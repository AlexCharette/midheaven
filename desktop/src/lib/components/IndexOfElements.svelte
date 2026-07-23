<script lang="ts">
  import { fade, fly, slide } from "svelte/transition";
  import { expoOut } from "svelte/easing";
  import type { ChartData } from "$lib/types";
  import { catOf, degInSign, planetById, relatedTo, signAt, textGlyph } from "$lib/types";
  import { app, focusedTag, houseSuffix, peek, selected, toggle, unpeek } from "$lib/state.svelte";
  import { swapDuration } from "$lib/motion";

  let { chart }: { chart: ChartData } = $props();

  // The legend cross-lights with the wheel: the focused element (a pin locks
  // it, else the hovered one) and everything it relates to are marked here too,
  // and hovering a row lights the orrery back.
  const focusTag = $derived(focusedTag());
  const related = $derived(focusTag ? relatedTo(chart, focusTag) : new Set<string>());

  const planetName = (id: string) => planetById(chart, id)?.name ?? id;

  // House entries show the ordinal without the trailing "house" word; the
  // suffix to strip is language-specific and comes from the backend (i18n),
  // so the mapping lives in one place. Empty until the locale list loads.
  const suffix = $derived(houseSuffix(chart.meta.locale ?? "en"));

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
        name: h.name.replace(suffix, ""),
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

<div class="index-fold">
  <button
    type="button"
    class="rubric index-summary"
    class:open={app.indexOpen}
    aria-expanded={app.indexOpen}
    aria-controls="index-body"
    onclick={() => (app.indexOpen = !app.indexOpen)}
  >
    <span class="head-group">
      <span class="lbl">Index of Elements</span>
      <span class="caret" aria-hidden="true">
        <svg width="11" height="7" viewBox="0 0 11 7"><path d="M1 1.2 L5.5 5.5 L10 1.2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </span>
    </span>
  </button>
  {#if app.indexOpen}
    <div class="index" id="index-body" transition:slide={{ duration: swapDuration(), easing: expoOut }}>
    {#each columns as col (col.head)}
      {@const filtering = col.filterable && !expanded[col.head]}
      {@const shown = filtering
        ? col.entries.filter((e) => occupied.has(e.tag) || selected.has(e.tag))
        : col.entries}
      {@const hidden = col.entries.length - shown.length}
      <div class="band">
        <h3>{col.head}</h3>
        <div class="entries">
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
              in:fly|local={{ y: 4, duration: swapDuration(), easing: expoOut }}
              out:fade|local={{ duration: swapDuration() }}
            >
              <span class="g g-{catOf(e.tag)}">{textGlyph(e.glyph)}</span>
              <span class="nm">{e.name}</span>
              {#if e.detail}<span class="dt">{e.detail}</span>{/if}
            </button>
          {/each}
          {#if filtering && hidden > 0}
            <button class="more" onclick={() => (expanded[col.head] = true)}>· {hidden} more</button>
          {:else if col.filterable && expanded[col.head]}
            <button class="more" onclick={() => (expanded[col.head] = false)}>· fewer</button>
          {/if}
        </div>
      </div>
    {/each}
    </div>
  {/if}
</div>

<style>
  /* the whole index folds away to hand the commentary the full panel.
     minimal button reset — the .rubric class still supplies the flanking
     rules, small-caps, letter-spacing, size, colour and bottom margin. */
  .index-summary {
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    font-family: inherit;
    cursor: pointer;
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
  .index-summary:not(.open) {
    margin-bottom: 0.4rem;
  }
  .index-summary:not(.open) .caret {
    transform: rotate(-90deg);
  }
  @media (prefers-reduced-motion: no-preference) {
    .index-summary .caret {
      transition: transform var(--dur-fast) var(--ease-out-quint);
    }
  }
  /* stacked, full-width category bands — entries flow horizontally and wrap,
     so the panel keeps a fixed width instead of a tall right-hand column */
  .index {
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
    margin-bottom: 1.4rem;
  }
  .band {
    min-width: 0;
  }
  .entries {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.1rem 0.9rem;
    min-width: 0;
  }
  h3 {
    font-weight: 400;
    font-style: italic;
    font-size: 0.98rem;
    color: var(--ink-3);
    margin: 0 0 0.4rem;
    padding-bottom: 0.25rem;
    border-bottom: 1px solid var(--line);
  }
  .entry {
    display: inline-flex;
    align-items: baseline;
    gap: 0.4em;
    text-align: left;
    padding: 0.12rem 0.35rem;
    border-radius: 3px;
    font-size: 0.92rem;
    color: var(--ink);
    transition: background var(--dur-fast) var(--ease-out-quint);
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
    width: auto;
    text-align: left;
  }
  .entry .nm {
    white-space: nowrap;
    border-bottom: 1px solid transparent;
    transition: border-color var(--dur-fast) var(--ease-out-quint);
  }
  .entry:hover .nm,
  .entry.rel .nm {
    border-bottom-color: var(--hairline);
  }
  .entry[aria-pressed="true"] {
    background: var(--ink-a04);
  }
  .entry[aria-pressed="true"] .nm {
    border-bottom-color: var(--ink-2);
  }
  /* cross-lit from the wheel: the focused row brightens, its relations mark */
  .entry.focus {
    background: var(--ink-a04);
    color: var(--ink);
  }
  .entry.focus .nm {
    border-bottom-color: var(--ink);
  }
  .entry .dt {
    flex: none;
    color: var(--ink-3);
    font-size: 0.85em;
    font-variant-numeric: tabular-nums;
  }
  .more {
    padding: 0.12rem 0.35rem;
    font-size: 0.82rem;
    font-style: italic;
    color: var(--ink-3);
  }
  .more:hover {
    color: var(--ink);
    text-decoration: underline;
  }
</style>
