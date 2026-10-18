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
    } catch (e) {
      // A reading with no store behind it simply has nothing beside it.
      beside = { standing: [], aboutPerson: [] };
      err = reason(e);
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
    <h2 class="rubric">written before</h2>

    {#if err}
      <p class="apparatus-text err">{err}</p>
    {/if}

    {#if beside.aboutPerson.length > 0}
      <div class="group">
        <h3 class="apparatus-text">about them</h3>
        {#each beside.aboutPerson as n (n.id)}
          <article class="prior"><blockquote>{n.text}</blockquote></article>
        {/each}
      </div>
    {/if}

    {#if focus.length > 0}
      <div class="group">
        <h3 class="apparatus-text">on {focusLabel}</h3>
        {#each forFocus as n (n.id)}
          <article class="prior"><blockquote>{n.text}</blockquote></article>
        {:else}
          <p class="apparatus-text none">Nothing written yet.</p>
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
              <span class="apparatus-text">filed under {focusLabel}</span>
              <span class="grow"></span>
              <button type="button" class="ghost" onclick={() => (composing = false)}>cancel</button>
              <button type="button" class="frame-btn" disabled={saving} onclick={keep}>
                {saving ? "keeping…" : "keep"}
              </button>
            </div>
          </div>
        {:else}
          <button type="button" class="ghost add" onclick={() => (composing = true)}>
            write one…
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
    margin: 0;
    font-size: 0.6rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-3);
    font-weight: 400;
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
  .none {
    color: var(--ink-3);
    font-style: italic;
  }
  .add {
    align-self: flex-start;
  }
  .compose {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border: 1px solid var(--line);
    border-top: 2px solid var(--brass);
    padding: 0.6rem;
  }
  .compose textarea {
    width: 100%;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--hairline);
    font: inherit;
    line-height: 1.6;
    padding: 0.2rem;
    resize: vertical;
  }
  .foot {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }
  .grow {
    flex: 1;
  }
  .err {
    color: var(--oxblood);
  }
</style>
