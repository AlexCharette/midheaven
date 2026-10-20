<script lang="ts">
  // What sits beside a reading without belonging to it.
  //
  // Two populations, two rules, and the difference is cardinality rather
  // than taste. Observations about the person in front of you are few and
  // always relevant, so they show throughout. Craft notes are many — you may
  // have three hundred about Saturn — so they appear only for the element
  // currently in focus, which the reading view already has a mechanism for:
  // pin Saturn on the wheel, or hover it in the index.
  //
  // Nothing here is exported. `save_artifact` and `save_pdf` render through
  // `notes::under`, which is chart-anchored only — a practitioner's craft,
  // and their notes about a client, are not part of that client's keepsake.
  import { listBeside, writeStandingNote } from "$lib/api";
  import { focusedTag, pinCount, pinnedTags } from "$lib/focus.svelte";
  import { matching } from "$lib/focus";
  import { notify } from "$lib/toasts.svelte";
  import { reason } from "$lib/failure";
  import type { Beside, ChartData, Note } from "$lib/types";
  import { elementsOf } from "$lib/types";
  import { onMount } from "svelte";

  let { chart }: { chart: ChartData } = $props();

  let beside = $state<Beside>({ standing: [], aboutPerson: [] });
  let err = $state("");
  let composing = $state(false);
  let draft = $state("");
  let saving = $state(false);

  const names = $derived(new Map(elementsOf(chart).map((e) => [e.tag, e.name])));

  onMount(() => void refresh());

  async function refresh() {
    try {
      beside = await listBeside();
    } catch {
      // A reading with no store behind it simply has nothing beside it.
      // Not an error worth showing: this is apparatus, and "no folder
      // chosen yet" is a normal state of the app, not a failure of it.
      beside = { standing: [], aboutPerson: [] };
    }
  }

  // The focus, as the section reads it: pins first, else the hovered
  // element. Unlike the commentary, an empty focus shows *nothing* — the
  // whole library is not an answer to "what have I written about this".
  const focus = $derived(pinCount() > 0 ? pinnedTags() : ([focusedTag()].filter(Boolean) as string[]));
  const forFocus = $derived(focus.length === 0 ? [] : matching(beside.standing, focus, "any"));
  const focusLabel = $derived(focus.map((t) => names.get(t) ?? t).join(" · "));

  async function keep() {
    const words = draft.trim();
    if (!words) return;
    saving = true;
    try {
      // The pins are already the tags — the same grammar the Composer uses,
      // and the reason writing from here costs no new way to name elements.
      await writeStandingNote(words, focus);
      draft = "";
      composing = false;
      await refresh();
      notify("standing note kept");
    } catch (e) {
      err = reason(e);
    } finally {
      saving = false;
    }
  }
</script>

{#if beside.aboutPerson.length > 0 || focus.length > 0}
  <section class="beside">
    <h2 class="section-head">Written before</h2>

    {#if err}
      <p class="err">{err}</p>
    {/if}

    {#if beside.aboutPerson.length > 0}
      <div class="group">
        <h3>About them</h3>
        {#each beside.aboutPerson as n (n.id)}
          <article class="prior"><blockquote>{n.text}</blockquote></article>
        {/each}
      </div>
    {/if}

    {#if focus.length > 0}
      <div class="group">
        <h3>On {focusLabel}</h3>
        {#each forFocus as n (n.id)}
          <article class="prior"><blockquote>{n.text}</blockquote></article>
        {:else}
          <p class="none">Nothing written yet.</p>
        {/each}

        {#if composing}
          <div class="compose">
            <textarea
              bind:value={draft}
              rows="3"
              placeholder={`what you know about ${focusLabel}`}
              aria-label="a standing note"
            ></textarea>
            <div class="foot">
              <span class="hint">Filed under {focusLabel}</span>
              <span class="grow"></span>
              <button type="button" class="btn bare small" onclick={() => (composing = false)}>Cancel</button>
              <button type="button" class="btn primary small" disabled={saving} onclick={keep}>
                {saving ? "Keeping…" : "Keep"}
              </button>
            </div>
          </div>
        {:else}
          <button type="button" class="btn quiet small add" onclick={() => (composing = true)}>
            Write one
          </button>
        {/if}
      </div>
    {/if}
  </section>
{/if}

<style>
  .beside {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    margin-top: 1.6rem;
    padding-top: 1.1rem;
    border-top: 1px solid var(--line);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  h3 {
    margin: 0.3rem 0 0;
    font-family: var(--font-sans);
    font-size: 0.78rem;
    font-weight: 500;
    color: var(--ink-2);
  }
  /* Recessive against the commentary: these are apparatus around the
     reading, not part of it. The verbatim words keep the finest setting. */
  .prior blockquote {
    margin: 0;
    padding: 0.1rem 0 0.1rem 0.7rem;
    border-left: 1px solid var(--line);
    color: var(--ink-2);
    font-size: 0.92rem;
    line-height: 1.6;
  }
  .none,
  .hint {
    font-family: var(--font-sans);
    font-size: 0.78rem;
    color: var(--ink-3);
  }
  .add {
    align-self: flex-start;
  }
  /* An opening in the stream rather than a card: the field is the only
     surface, so the note being written reads as the one thing in hand. */
  .compose {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .compose textarea {
    width: 100%;
    background: var(--surface);
    color: var(--ink);
    border: none;
    border-radius: var(--radius);
    box-shadow: inset 0 0 0 1px var(--line);
    font: inherit;
    line-height: 1.6;
    padding: 0.5rem 0.75rem;
    resize: vertical;
  }
  .compose textarea:focus {
    outline: none;
    box-shadow: inset 0 0 0 1px var(--brass), 0 0 0 3px var(--brass-wash);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .grow {
    flex: 1;
  }
  .err {
    font-family: var(--font-sans);
    font-size: 0.8rem;
    color: var(--oxblood);
  }
</style>
