<script lang="ts">
  import { correctExcerpt, mergeUp } from "$lib/api";
  import type { ChartData, Excerpt } from "$lib/types";
  import { catOf, elementsOf, textGlyph } from "$lib/types";
  import { app, selected, toggle } from "$lib/state.svelte";

  let { chart, visible }: { chart: ChartData; visible: Excerpt[] } = $props();

  const lookup = $derived(new Map(elementsOf(chart).map((e) => [e.tag, e])));

  // Curation only when unfiltered: adjacency in the visible list then equals
  // adjacency in the chart's list, so "merge ↑" is unambiguous.
  const curatable = $derived(selected.size === 0 && app.busy === false);

  let editing = $state<string | null>(null);
  let draft = $state("");
  let original = "";

  async function join(id: string) {
    try {
      app.chart = await mergeUp(id);
      app.status = "two passages joined";
    } catch (e) {
      app.status = `✗ ${e}`;
    }
  }

  function beginAmend(ex: Excerpt) {
    editing = ex.id;
    draft = ex.text;
    original = ex.text;
  }

  async function saveAmend() {
    if (editing === null) return;
    const id = editing;
    editing = null;
    if (draft.trim() === original) return;
    try {
      app.chart = await correctExcerpt(id, draft);
      app.status = "passage amended — re-sectioned";
    } catch (e) {
      app.status = `✗ ${e}`;
    }
  }

  function amendKeys(e: KeyboardEvent) {
    if (e.key === "Escape") {
      editing = null;
      e.preventDefault();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      saveAmend();
      e.preventDefault();
    }
  }
</script>

<h2 class="rubric">Commentary</h2>
{#if visible.length === 0}
  <p class="empty apparatus-text">
    {chart.excerpts.length === 0
      ? "no transcript passages were routed to this chart"
      : "no passage touches the selection — clear it to see everything"}
  </p>
{/if}
{#each visible as ex, i (ex.id)}
  <article class="passage">
    <div class="folio">
      {ex.time || "—"}
      {#if curatable && i > 0}
        <button class="curate" title="join this passage to the previous one" onclick={() => join(ex.id)}
          >merge ↑</button
        >
      {/if}
    </div>
    {#if editing === ex.id}
      <!-- svelte-ignore a11y_autofocus -->
      <textarea
        class="amend-box"
        bind:value={draft}
        rows={Math.max(2, Math.ceil(draft.length / 70))}
        autofocus
        onkeydown={amendKeys}
        onblur={saveAmend}
      ></textarea>
    {:else}
      <blockquote>
        “{ex.text}”
        {#if curatable}
          <button class="curate amend" title="correct the transcription" onclick={() => beginAmend(ex)}
            >amend</button
          >
        {/if}
      </blockquote>
    {/if}
    <div class="refs">
      <span class="apparatus-text">vide</span>
      {#each ex.tags as tag, i (tag)}
        {@const el = lookup.get(tag)}
        {#if i > 0}<span class="sep"> · </span>{/if}
        <button class="ref" aria-pressed={selected.has(tag)} onclick={() => toggle(tag)}>
          <span class="g g-{catOf(tag)}">{textGlyph(el?.glyph ?? "")}</span>
          <span class="nm">{el?.name ?? tag}</span>
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
  .curate {
    display: block;
    margin-left: auto;
    font-size: 0.78rem;
    font-style: italic;
    color: var(--ink-3);
    opacity: 0;
    transition: opacity 0.15s ease-out;
  }
  .passage:hover .curate {
    opacity: 1;
  }
  @media (prefers-reduced-motion: reduce) {
    .curate {
      transition: none;
    }
  }
  .curate:hover {
    color: var(--ink);
    text-decoration: underline;
  }
  blockquote .curate.amend {
    display: inline;
    margin-left: 0.6em;
    white-space: nowrap;
  }
  .amend-box {
    grid-column: 2;
    font: inherit;
    font-size: 1.04rem;
    line-height: 1.75;
    color: var(--ink);
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--hairline);
    resize: vertical;
    max-width: 62ch;
    padding: 0;
  }
  .amend-box:focus {
    outline: none;
    border-bottom-color: var(--brass);
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
