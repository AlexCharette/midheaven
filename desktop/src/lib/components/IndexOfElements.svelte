<script lang="ts">
  import { fade, fly, slide } from "svelte/transition";
  import { expoOut } from "svelte/easing";
  import type { ChartData } from "$lib/types";
  import { catOf, planetById, signOf, textGlyph } from "$lib/types";
  import { occupiedTags, relatedTo } from "$lib/focus";
  import { focusedTag, isPinned, peek, toggle, unpeek } from "$lib/focus.svelte";
  import { houseSuffix } from "$lib/options.svelte";
  import { fmt, t } from "$lib/chrome.svelte";
  import { swapDuration } from "$lib/motion";

  let { chart }: { chart: ChartData } = $props();

  // The legend cross-lights with the wheel: the focused element (a pin locks
  // it, else the hovered one) and everything it relates to are marked here too,
  // and hovering a row lights the orrery back.
  const focusTag = $derived(focusedTag());
  const related = $derived(focusTag ? relatedTo(chart, focusTag) : new Set<string>());

  const planetName = (id: string) => planetById(chart, id)?.name ?? id;

  // The chrome's "· 6 more" carries the separator the artifact sets it after;
  // here it stands as its own control and the dot is noise.
  const bare = (s: string) => s.replace(/^\s*·\s*/, "");

  // House entries show the ordinal without the trailing "house" word; the
  // suffix to strip is language-specific and comes from the backend (i18n),
  // so the mapping lives in one place. Empty until the locale list loads.
  const suffix = $derived(houseSuffix(chart.meta.locale ?? "en"));

  // Relevance rule (canonical prose lives in templates/reading.html beside
  // syncRelevance; keep the two in step): visible = occupied ∪ pinned ∪
  // expanded — occupancy means a body stands in the sign/house, and a pinned
  // filter must never hide itself.
  const occupied = $derived(occupiedTags(chart));
  let expanded = $state<Record<string, boolean>>({ signs: false, houses: false });

  // This fold is nobody else's business, so it is local state rather than a
  // field on a shared object. Folded by default so the orrery leads; opening
  // it reveals the full keyboard-accessible mirror.
  // Open by default: as a panel competing with the commentary this was
  // noise and shut was right, but it hangs off the plate now as that
  // figure's key — and a key that starts closed is a key nobody reads.
  let open = $state(true);

  interface Entry {
    tag: string;
    glyph: string;
    name: string;
    detail: string;
  }
  interface Column {
    /** The key the fold state is stored under — not shown; `label` is. */
    head: string;
    label: string;
    entries: Entry[];
    filterable?: boolean;
  }
  const columns: Column[] = $derived([
    {
      head: "planets",
      label: t().bands[0],
      entries: chart.planets.map((p) => ({
        tag: p.id,
        glyph: p.glyph,
        name: p.name,
        detail: `${p.deg}° ${textGlyph(signOf(chart, p).glyph)}`,
      })),
    },
    {
      head: "signs",
      label: t().bands[1],
      filterable: true,
      entries: chart.signs.map((s) => ({ tag: s.id, glyph: s.glyph, name: s.name, detail: "" })),
    },
    {
      head: "houses",
      label: t().bands[2],
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
      label: t().bands[3],
      entries: chart.aspects.map((a) => ({
        tag: a.id,
        glyph: a.glyph,
        name: `${planetName(a.a)} – ${planetName(a.b)}`,
        detail: "",
      })),
    },
    {
      head: "kinds",
      label: t().bands[4],
      entries: chart.kinds.map((k) => ({
        tag: k.id,
        glyph: k.glyph,
        name: k.name,
        detail: "",
      })),
    },
  ]);
</script>

<div class="index-fold">
  <button
    type="button"
    class="section-head index-summary"
    class:open={open}
    aria-expanded={open}
    aria-controls="index-body"
    onclick={() => (open = !open)}
  >
    <span class="head-group">
      <span class="lbl">{t().indexOfElements}</span>
      <span class="caret" aria-hidden="true">
        <svg width="11" height="7" viewBox="0 0 11 7"><path d="M1 1.2 L5.5 5.5 L10 1.2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </span>
    </span>
  </button>
  {#if open}
    <div class="index" id="index-body" transition:slide={{ duration: swapDuration(), easing: expoOut }}>
    {#each columns as col (col.head)}
      {@const filtering = col.filterable && !expanded[col.head]}
      {@const shown = filtering
        ? col.entries.filter((e) => occupied.has(e.tag) || isPinned(e.tag))
        : col.entries}
      {@const hidden = col.entries.length - shown.length}
      <div class="band">
        <h3>{col.label}</h3>
        <div class="entries">
          {#each shown as e (e.tag)}
            <button
              class="entry"
              class:focus={focusTag === e.tag}
              class:rel={related.has(e.tag)}
              aria-pressed={isPinned(e.tag)}
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
            <button class="more" onclick={() => (expanded[col.head] = true)}>{bare(fmt(t().more, { n: hidden }))}</button>
          {:else if col.filterable && expanded[col.head]}
            <button class="more" onclick={() => (expanded[col.head] = false)}>{bare(t().fewer)}</button>
          {/if}
        </div>
      </div>
    {/each}
    </div>
  {/if}
</div>

<style>
  /* the whole index folds away to hand the commentary the full panel; its
     head is the section head, made a toggle */
  .index-summary {
    display: flex;
    padding: 0.2rem 0;
    border: 0;
    background: none;
    cursor: pointer;
  }
  .index-summary:hover {
    color: var(--ink-2);
  }
  .index-summary:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: 2px;
    border-radius: 3px;
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
    gap: 0.9rem;
    margin-bottom: 1.4rem;
  }
  /* A band reads as a labelled row: the category in a narrow gutter, its
     entries flowing beside it, so five bands scan as one table rather than
     five headed lists. */
  .band {
    display: grid;
    grid-template-columns: 4.6rem 1fr;
    min-width: 0;
    gap: 0 0.6rem;
    align-items: baseline;
  }
  .entries {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.15rem 0.15rem;
    min-width: 0;
    margin-left: -0.45rem;
  }
  h3 {
    margin: 0;
    padding-top: 0.2rem;
    font-family: var(--font-sans);
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--ink-3);
  }
  .entry {
    display: inline-flex;
    align-items: baseline;
    gap: 0.4em;
    text-align: left;
    padding: 0.18rem 0.45rem;
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    font-size: 0.82rem;
    color: var(--ink-2);
    transition:
      background var(--dur-fast) var(--ease-out-quint),
      color var(--dur-fast) var(--ease-out-quint);
  }
  .entry:hover {
    background: var(--surface-hover);
    color: var(--ink);
  }
  .entry:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: 1px;
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
  }
  /* cross-lit from the wheel: the focused entry brightens, its relations
     lift to full ink; a pinned one holds the brass selection */
  .entry.rel {
    color: var(--ink);
  }
  .entry.focus {
    background: var(--surface-hover);
    color: var(--ink);
  }
  .entry[aria-pressed="true"] {
    background: var(--brass-wash);
    color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--brass-halo);
  }
  .entry .dt {
    flex: none;
    color: var(--ink-3);
    font-size: 0.85em;
    font-variant-numeric: tabular-nums;
  }
  .more {
    padding: 0.18rem 0.45rem;
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    font-size: 0.78rem;
    color: var(--brass);
  }
  .more:hover {
    background: var(--surface-hover);
  }
</style>
