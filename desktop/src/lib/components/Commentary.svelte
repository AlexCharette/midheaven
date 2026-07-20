<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { addExcerpt, correctExcerpt, deleteExcerpt, mergeUp } from "$lib/api";
  import { SvelteSet } from "svelte/reactivity";
  import type { ChartData, Excerpt } from "$lib/types";
  import { catOf, elementsOf, textGlyph } from "$lib/types";
  import { app, selected, toggle } from "$lib/state.svelte";

  let { chart, visible }: { chart: ChartData; visible: Excerpt[] } = $props();

  const elements = $derived(elementsOf(chart));
  const lookup = $derived(new Map(elements.map((e) => [e.tag, e])));

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

  // one key-chord contract for both textareas: Esc cancels, ⌘/Ctrl-Enter commits
  const escCommit = (cancel: () => void, commit: () => void) => (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      cancel();
      e.preventDefault();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      commit();
      e.preventDefault();
    }
  };
  const amendKeys = escCommit(() => (editing = null), saveAmend);
  const rowsFor = (t: string) => Math.max(2, Math.ceil(t.length / 70));

  async function remove(exId: string, preview: string) {
    const sure = await ask(`Remove this passage?\n\n“${preview}”\n\nIts words are not recoverable.`, {
      title: "Midheaven",
      kind: "warning",
    });
    if (!sure) return;
    try {
      app.chart = await deleteExcerpt(exId);
      app.status = "passage removed";
    } catch (e) {
      app.status = `✗ ${e}`;
    }
  }

  // ---- manual passage composer ----
  let composing = $state(false);
  let draftText = $state("");
  const draftTags = new SvelteSet<string>();

  function openComposer() {
    composing = true;
    draftText = "";
    draftTags.clear();
  }

  async function fileIt() {
    if (!draftText.trim()) return;
    composing = false;
    try {
      app.chart = await addExcerpt(draftText, [...draftTags]);
      app.status = "passage added";
    } catch (e) {
      app.status = `✗ ${e}`;
    }
  }

  const composerKeys = escCommit(() => (composing = false), fileIt);
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
      {#if curatable}
        <button
          class="curate destructive"
          title="remove this passage (asks first)"
          onclick={() => remove(ex.id, ex.text.length > 80 ? ex.text.slice(0, 80) + "…" : ex.text)}
          >remove</button
        >
      {/if}
    </div>
    {#if editing === ex.id}
      <!-- svelte-ignore a11y_autofocus -->
      <textarea
        class="amend-box"
        bind:value={draft}
        rows={rowsFor(draft)}
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

{#if curatable}
  <div class="composer">
    {#if composing}
      <!-- svelte-ignore a11y_autofocus -->
      <textarea
        class="compose-box"
        bind:value={draftText}
        rows={rowsFor(draftText)}
        autofocus
        placeholder="a passage in the astrologer's words…"
        onkeydown={composerKeys}
      ></textarea>
      <div class="tag-row">
        {#each elements as el (el.tag)}
          <button
            class="ref"
            aria-pressed={draftTags.has(el.tag)}
            onclick={() => (draftTags.has(el.tag) ? draftTags.delete(el.tag) : draftTags.add(el.tag))}
          >
            <span class="g g-{catOf(el.tag)}">{textGlyph(el.glyph)}</span>
            <span class="nm">{el.name}</span>
          </button>
        {/each}
      </div>
      <div class="composer-actions">
        <span class="apparatus-text">
          {draftTags.size === 0 ? "no tags picked — the router will file it from the words" : `${draftTags.size} tags picked`}
        </span>
        <button class="ghost" onclick={() => (composing = false)}>discard</button>
        <button class="frame-btn" onclick={fileIt} disabled={!draftText.trim()}>file it</button>
      </div>
    {:else}
      <button class="ghost open-composer" onclick={openComposer}>✎ add a passage</button>
    {/if}
  </div>
{/if}

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
  .amend-box,
  .compose-box {
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
  .amend-box {
    grid-column: 2;
  }
  .compose-box {
    width: 100%;
  }
  .amend-box:focus,
  .compose-box:focus {
    outline: none;
    border-bottom-color: var(--brass);
  }
  .curate.destructive:hover {
    color: var(--oxblood);
  }
  .composer {
    margin-top: 1rem;
    padding-top: 0.8rem;
    border-top: 1px solid var(--line);
  }
  .open-composer {
    display: block;
    margin: 0 auto;
    font-size: 0.88rem;
  }
  .tag-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 0.8rem;
    margin: 0.6rem 0;
    font-size: 0.85rem;
  }
  .composer-actions {
    display: flex;
    align-items: baseline;
    gap: 1.2rem;
    font-size: 0.88rem;
  }
  .composer-actions .apparatus-text {
    margin-right: auto;
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
