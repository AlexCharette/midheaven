<script lang="ts">
  // Writing a note over the sky on the plate.
  //
  // It sits in the margin column rather than over the plate, because the plate
  // is how you say what the note is about: while this is open the wheel and the
  // element index become an index — click Saturn, click the fifth house — and
  // the pins are the note's tags. A panel covering them would have needed a
  // second way to name elements in an app that has one.
  import { writeMomentNote } from "$lib/api";
  import { pinnedTags, resetFocus } from "$lib/focus.svelte";
  import { anchorFor } from "$lib/journal";
  import { settled, whenSettled } from "$lib/preview.svelte";
  import { ensureReadingsDir } from "$lib/readings";
  import { notify } from "$lib/toasts.svelte";
  import { elementsOf } from "$lib/types";
  import { reason } from "$lib/failure";

  let { onclose }: { onclose: () => void } = $props();

  let text = $state("");
  let saving = $state(false);
  let error = $state("");

  const tags = $derived(pinnedTags());
  // Names come from the settled chart, which is the same chart the tags were
  // validated against — so a chip can never name something the note could not
  // be filed under.
  const names = $derived(
    new Map(settled() ? elementsOf(settled()!.chart).map((e) => [e.tag, e.name]) : []),
  );

  async function save() {
    error = "";
    const words = text.trim();
    if (!words) {
      error = "a note needs words";
      return;
    }
    saving = true;
    try {
      // A note is kept, so this is a moment a readings folder is earned.
      if (!(await ensureReadingsDir())) {
        error = "choose a folder to keep notes in";
        return;
      }
      // The sky may still be moving — a keystroke in the date field a moment
      // ago is a recompute in flight. Anchor to what it settles on, not to the
      // frame that happened to be on the plate.
      if (!(await whenSettled())) {
        error = "this moment does not compute — the note has no sky to file under";
        return;
      }
      const now = settled();
      if (!now) {
        error = "no chart to file this under yet";
        return;
      }
      await writeMomentNote(words, tags, anchorFor(now.input, now.chart));
      notify(tags.length ? `note filed under ${tags.length} element${tags.length > 1 ? "s" : ""}` : "note kept");
      text = "";
      resetFocus();
      onclose();
    } catch (e) {
      error = reason(e);
    } finally {
      saving = false;
    }
  }

  function cancel() {
    resetFocus();
    onclose();
  }
</script>

<div class="composer">
  <p class="rubric">a note on this sky</p>
  <textarea
    bind:value={text}
    rows="4"
    placeholder="what you want to remember about this moment"
    aria-label="note"
  ></textarea>

  <div class="tags" aria-live="polite">
    {#if tags.length === 0}
      <span class="hint">click the wheel or the index to file it under an element — or leave it unfiled</span>
    {:else}
      {#each tags as tag (tag)}
        <span class="chip">{names.get(tag) ?? tag}</span>
      {/each}
    {/if}
  </div>

  {#if error}
    <p class="err" aria-live="polite">{error}</p>
  {/if}

  <div class="acts">
    <button type="button" class="ghost" onclick={cancel}>cancel</button>
    <button type="button" class="frame-btn" onclick={save} disabled={saving}>
      {saving ? "keeping…" : "keep note"}
    </button>
  </div>
</div>

<style>
  .composer {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding-top: 0.7rem;
    border-top: 1px solid var(--line);
  }
  /* Written on a ruled line, not into a box — the same treatment every
     other field on this screen gets. */
  textarea {
    width: 100%;
    resize: vertical;
    padding: 0.3rem 0 0.4rem;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--line);
    font: inherit;
    line-height: 1.5;
  }
  textarea:focus {
    border-bottom-color: var(--hairline);
  }
  textarea:focus-visible {
    outline: none;
    border-bottom-color: var(--brass);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    min-height: 1.3rem;
  }
  /* A tag reads as a struck mark rather than a pill: underlined in its
     category's ink, which is also how the wheel shows the same selection. */
  .chip {
    padding: 0.05rem 0.1rem;
    font-size: 0.8rem;
    color: var(--ink-2);
    border-bottom: 1px solid var(--hairline);
  }
  .hint,
  .err {
    font-size: 0.8rem;
    font-style: italic;
    color: var(--ink-3);
    text-wrap: pretty;
  }
  .err {
    color: var(--oxblood-ink, var(--ink-2));
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
  }
</style>
