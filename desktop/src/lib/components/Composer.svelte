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

<div class="composer form">
  <h2 class="section-head">A note on this sky</h2>
  <textarea
    bind:value={text}
    rows="4"
    placeholder="What you want to remember about this moment"
    aria-label="note"
  ></textarea>

  <div class="tags" aria-live="polite">
    {#if tags.length === 0}
      <span class="hint">Click the wheel to file it under an element — or leave it unfiled.</span>
    {:else}
      {#each tags as tag (tag)}
        <span class="chip">{names.get(tag) ?? tag}</span>
      {/each}
    {/if}
  </div>

  {#if error}
    <p class="error" aria-live="polite">{error}</p>
  {/if}

  <div class="acts">
    <button type="button" class="btn bare small" onclick={cancel}>Cancel</button>
    <button type="button" class="btn primary small" onclick={save} disabled={saving}>
      {saving ? "Keeping…" : "Keep note"}
    </button>
  </div>
</div>

<style>
  .composer {
    gap: 0.6rem;
    padding-top: 1.2rem;
    border-top: 1px solid var(--line);
  }
  .composer .section-head {
    margin: 0;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    min-height: 1.6rem;
  }
  /* what the note will be filed under, lit the way a pinned index entry is */
  .chip {
    display: inline-flex;
    align-items: center;
    height: 1.6rem;
    padding: 0 0.55rem;
    border-radius: var(--radius-sm);
    background: var(--brass-wash);
    box-shadow: inset 0 0 0 1px var(--brass-halo);
    font-size: 0.78rem;
    color: var(--ink);
  }
  .hint {
    font-size: 0.78rem;
    color: var(--ink-3);
    text-wrap: pretty;
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    gap: 0.4rem;
  }
</style>
