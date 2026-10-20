<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { deleteNote, mergeUp, writeNote } from "$lib/api";
  import { SvelteSet } from "svelte/reactivity";
  import type { ChartData, Note } from "$lib/types";
  import { catOf, elementsOf, textGlyph } from "$lib/types";
  import { notes, refreshNotes, setNotes } from "$lib/notes.svelte";
  import { isBusy } from "$lib/busy.svelte";
  import { hoveredTag, isPinned, mode, pinCount, toggle, touchesPins } from "$lib/focus.svelte";
  import { notify } from "$lib/toasts.svelte";
  import { fmt, t } from "$lib/chrome.svelte";
  import { reason } from "$lib/failure";

  let { chart, visible }: { chart: ChartData; visible: Note[] } = $props();

  const elements = $derived(elementsOf(chart));
  const lookup = $derived(new Map(elements.map((e) => [e.tag, e])));

  // merge ↑ joins into the previous *visible* note, so it's only
  // unambiguous when nothing filters the list — no selection and no hover
  // preview. amend/remove/add are id-based and safe under any filter, so they
  // stay available whenever the app isn't busy.
  const mergeable = $derived(pinCount() === 0 && hoveredTag() === null && !isBusy());
  const editable = $derived(!isBusy());

  let editing = $state<string | null>(null);
  let draft = $state("");
  let original = "";

  async function join(id: string) {
    try {
      setNotes(await mergeUp(id));
      notify("two notes joined");
    } catch (e) {
      notify(reason(e), "error");
    }
  }

  function beginAmend(ex: Note) {
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
      setNotes(await writeNote(draft, [], id));
      notify("note amended — re-sectioned");
    } catch (e) {
      notify(reason(e), "error");
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
    const sure = await ask(`Remove this note?\n\n“${preview}”\n\nIts words are not recoverable.`, {
      title: "Midheaven",
      kind: "warning",
    });
    if (!sure) return;
    try {
      await deleteNote(exId);
      await refreshNotes();
      notify("note removed");
    } catch (e) {
      notify(reason(e), "error");
    }
  }

  // ---- manual note composer ----
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
      setNotes(await writeNote(draftText, [...draftTags]));
      notify("note added");
    } catch (e) {
      notify(reason(e), "error");
    }
  }

  const composerKeys = escCommit(() => (composing = false), fileIt);
</script>

<!-- the reference chip shared by note refs and the composer's tag picker:
     a glyph + name toggle, its category coloring keyed off the tag -->
{#snippet refButton(tag: string, glyph: string, name: string, pressed: boolean, onToggle: () => void)}
  <button class="ref" aria-pressed={pressed} onclick={onToggle}>
    <span class="g g-{catOf(tag)}">{textGlyph(glyph)}</span>
    <span class="nm">{name}</span>
  </button>
{/snippet}

<h2 class="section-head">{t().commentary}</h2>
{#if visible.length === 0}
  <div class="empty-plate">
    <span class="mark" aria-hidden="true">✶</span>
    {#if notes().length === 0}
      <p class="caption">{t().emptyNoneFiled}</p>
      <p class="sub">Transcribe a session or add one by hand to begin the commentary.</p>
    {:else}
      <p class="caption">{fmt(t().emptyNoMatch, { word: mode() === "all" ? t().all : t().any })}</p>
      <p class="sub">Clear the selection to see the whole reading again.</p>
    {/if}
  </div>
{/if}
{#each visible as ex, i (ex.id)}
  {@const pinned = touchesPins(ex)}
  <article class="note" class:pinned>
    <div class="folio">
      {#if pinned}<span class="pin-mark astro" title="tied to the pinned selection">☞</span>{/if}
      {ex.at || "—"}
      {#if mergeable && i > 0}
        <button class="curate" title="join this note to the previous one" onclick={() => join(ex.id)}
          >merge ↑</button
        >
      {/if}
      {#if editable}
        <button
          class="curate destructive"
          title="remove this note (asks first)"
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
        {#if editable}
          <button class="curate amend" title="correct the transcription" onclick={() => beginAmend(ex)}
            >amend</button
          >
        {/if}
      </blockquote>
    {/if}
    <div class="refs">
      {#each ex.tags as tag (tag)}
        {@const el = lookup.get(tag)}
        {@render refButton(tag, el?.glyph ?? "", el?.name ?? tag, isPinned(tag), () => toggle(tag))}
      {/each}
    </div>
  </article>
{/each}

{#if editable}
  <div class="composer">
    {#if composing}
      <!-- svelte-ignore a11y_autofocus -->
      <textarea
        class="compose-box"
        bind:value={draftText}
        rows={rowsFor(draftText)}
        autofocus
        placeholder="what do you notice?"
        onkeydown={composerKeys}
      ></textarea>
      <div class="tag-row">
        {#each elements as el (el.tag)}
          {@render refButton(el.tag, el.glyph, el.name, draftTags.has(el.tag), () =>
            draftTags.has(el.tag) ? draftTags.delete(el.tag) : draftTags.add(el.tag))}
        {/each}
      </div>
      <div class="composer-actions">
        <span class="hint">
          {draftTags.size === 0 ? "no tags picked — the router will file it from the words" : `${draftTags.size} tags picked`}
        </span>
        <button class="btn bare small" onclick={() => (composing = false)}>Discard</button>
        <button class="btn primary small" onclick={fileIt} disabled={!draftText.trim()}>File it</button>
      </div>
    {:else}
      <button class="btn quiet small open-composer" onclick={openComposer}>
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
          ><path d="M3 13h2.4L12.6 5.8a1.7 1.7 0 0 0-2.4-2.4L3 10.6V13Z" /><path d="m9.3 4.3 2.4 2.4" /></svg
        >
        Add a note
      </button>
    {/if}
  </div>
{/if}

<style>
  .note {
    display: grid;
    grid-template-columns: 5.2rem 1fr;
    gap: 0 1.3rem;
    padding: 1.05rem 0.9rem;
    margin: 0 -0.9rem;
    border-radius: var(--radius);
    user-select: text;
    cursor: text;
  }
  .note + .note {
    box-shadow: 0 -1px 0 var(--line);
  }
  /* a pinned note is lit like the index entry it answers to: the same faint
     brass ground, and a manicule in the folio */
  .note.pinned {
    background: rgba(196, 154, 48, 0.07);
  }
  .note.pinned + .note,
  .note + .note.pinned {
    box-shadow: none;
  }
  @media (prefers-reduced-motion: no-preference) {
    .note {
      transition: background var(--dur-base) var(--ease-out-quint);
    }
  }
  /* Narrow widths cannot afford a fixed margin column: 5.2rem of folio out
     of 390 leaves the words about thirty characters, which is not a measure.
     The anchor goes above the line instead, where it costs nothing. */
  @media (max-width: 560px) {
    .note {
      grid-template-columns: 1fr;
      gap: 0.2rem;
    }
    .folio {
      text-align: left;
      padding-top: 0;
    }
    blockquote,
    .refs {
      grid-column: 1;
    }
  }
  .folio {
    color: var(--ink-3);
    font-family: var(--font-sans);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    padding-top: 0.38rem;
    text-align: right;
  }
  .note.pinned .folio {
    color: var(--ink-2);
  }
  .pin-mark {
    display: block;
    color: var(--brass);
    font-size: 0.82rem;
    margin-bottom: 0.1rem;
  }
  .curate {
    display: block;
    margin: 0.3rem 0 0 auto;
    font-family: var(--font-sans);
    font-size: 0.72rem;
    color: var(--ink-3);
    opacity: 0;
    transition: opacity 0.15s ease-out;
  }
  .note:hover .curate {
    opacity: 1;
  }
  @media (prefers-reduced-motion: reduce) {
    .curate {
      transition: none;
    }
  }
  .curate:hover {
    color: var(--ink);
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
    background: var(--surface);
    border: none;
    border-radius: var(--radius);
    box-shadow: inset 0 0 0 1px var(--line);
    resize: vertical;
    max-width: 62ch;
    padding: 0.55rem 0.8rem;
    transition: box-shadow var(--dur-fast) var(--ease-out-quint);
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
    box-shadow: inset 0 0 0 1px var(--brass), 0 0 0 3px var(--brass-wash);
  }
  .curate.destructive:hover {
    color: var(--oxblood);
  }
  .composer {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--line);
  }
  .tag-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0.7rem 0;
  }
  .composer-actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .hint {
    margin-right: auto;
    font-family: var(--font-sans);
    font-size: 0.78rem;
    color: var(--ink-3);
  }
  blockquote {
    margin: 0;
    grid-column: 2;
    font-size: 1.04rem;
    line-height: 1.75;
    max-width: 62ch;
    text-indent: -0.45em;
  }
  /* What a note is filed under, as the same marks the index uses: a coloured
     glyph and a word, no box. Each is a toggle — click one to see every note
     it shares. */
  .refs {
    grid-column: 2;
    display: flex;
    flex-wrap: wrap;
    gap: 0.15rem 0.2rem;
    margin: 0.5rem 0 0 -0.45rem;
    user-select: none;
    cursor: default;
  }
  .ref {
    display: inline-flex;
    align-items: baseline;
    gap: 0.35em;
    padding: 0.16rem 0.45rem;
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    font-size: 0.78rem;
    color: var(--ink-2);
    white-space: nowrap;
    transition:
      background var(--dur-fast) var(--ease-out-quint),
      color var(--dur-fast) var(--ease-out-quint);
  }
  .ref .g {
    font-family: var(--font-astro);
    font-size: 1.05em;
  }
  .ref .g.g-house {
    font-family: var(--font-serif);
    font-size: 0.9em;
  }
  .ref:hover {
    background: var(--surface-hover);
    color: var(--ink);
  }
  .ref[aria-pressed="true"] {
    background: var(--brass-wash);
    color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--brass-halo);
  }
  .ref:focus-visible {
    outline: 2px solid var(--brass-halo);
    outline-offset: 1px;
  }
</style>
