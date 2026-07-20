<script lang="ts">
  import type { ChartData } from "$lib/types";
  import { catOf, textGlyph } from "$lib/types";
  import { app, matches, selected, toggle } from "$lib/state.svelte";

  let { chart }: { chart: ChartData } = $props();

  const lookup = $derived(
    new Map(
      [
        ...chart.planets.map((x) => [x.id, { glyph: x.glyph, name: x.name }] as const),
        ...chart.signs.map((x) => [x.id, { glyph: x.glyph, name: x.name }] as const),
        ...chart.houses.map((x) => [x.id, { glyph: x.label, name: x.name }] as const),
        ...chart.aspects.map((x) => [x.id, { glyph: x.glyph, name: x.name }] as const),
      ],
    ),
  );
  const visible = $derived(chart.excerpts.filter(matches));
</script>

<h2 class="rubric">Commentary</h2>
{#if visible.length === 0}
  <p class="empty apparatus-text">
    {chart.excerpts.length === 0
      ? "no transcript passages were routed to this chart"
      : "no passage touches the selection — clear it to see everything"}
  </p>
{/if}
{#each visible as ex (ex.id)}
  <article class="passage">
    <div class="folio">{ex.time || "—"}</div>
    <blockquote>“{ex.text}”</blockquote>
    <div class="refs">
      <span class="apparatus-text">vide</span>
      {#each ex.tags as tag, i (tag)}
        {#if i > 0}<span class="sep"> · </span>{/if}
        <button class="ref" aria-pressed={selected.has(tag)} onclick={() => toggle(tag)}>
          <span class="g g-{catOf(tag)}">{textGlyph(lookup.get(tag)?.glyph ?? "")}</span>
          <span class="nm">{lookup.get(tag)?.name ?? tag}</span>
        </button>
      {/each}
    </div>
  </article>
{/each}

<style>
  .passage {
    display: grid;
    grid-template-columns: 5.2rem 1fr;
    gap: 0 1.3rem;
    padding: 1.05rem 0;
    user-select: text;
    cursor: text;
  }
  .passage + .passage {
    border-top: 1px solid var(--line);
  }
  .folio {
    color: var(--ink-3);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.06em;
    padding-top: 0.35rem;
    text-align: right;
  }
  blockquote {
    margin: 0;
    grid-column: 2;
    font-size: 1.04rem;
    line-height: 1.75;
    max-width: 62ch;
    text-indent: -0.45em;
  }
  .refs {
    grid-column: 2;
    margin-top: 0.45rem;
    font-size: 0.85rem;
    color: var(--ink-3);
    user-select: none;
    cursor: default;
  }
  .ref {
    color: var(--ink-2);
    white-space: nowrap;
  }
  .ref .g {
    margin-right: 0.3em;
  }
  .ref .nm {
    border-bottom: 1px dotted var(--line);
  }
  .ref:hover .nm {
    border-bottom: 1px solid var(--hairline);
    color: var(--ink);
  }
  .ref[aria-pressed="true"] .nm {
    border-bottom: 1px solid var(--ink-2);
    color: var(--ink);
  }
  .sep {
    color: var(--ink-3);
  }
  .empty {
    text-align: center;
    padding: 2rem 0;
  }
</style>
