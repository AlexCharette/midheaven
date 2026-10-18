<script lang="ts">
  // ─── SPIKE: the unbounded chart ────────────────────────────────────────
  //
  // The wheel with nothing around it. No plate frame, no app frame, no
  // margin column — the sky occupies the screen and everything else is
  // called into being by a click.
  //
  // What that costs, stated up front so it can be judged: the frame was
  // doing work. It said "this is a figure, and the things beside it are its
  // caption", which is how the reading view knows what it is. Without it the
  // wheel has to hold the page on its own draughtsmanship alone.
  //
  // What it buys: clicking an element is no longer "selecting a filter in a
  // panel", it is marking a point on a chart. So the draft does not open in
  // a sidebar — it rises at the foot of the sky, as a line written under the
  // thing it is about, the way an observer annotates a plate.
  import { onMount } from "svelte";
  import { lastPlace, preview, writeMomentNote } from "$lib/api";
  import { clearPins, pinnedTags, toggle } from "$lib/focus.svelte";
  import { anchorFor } from "$lib/journal";
  import { notify } from "$lib/toasts.svelte";
  import { reason } from "$lib/failure";
  import { nowMoment } from "$lib/civil";
  import { elementsOf } from "$lib/types";
  import type { ChartData, PlaceDto, PreviewInput } from "$lib/types";
  import Wheel from "$lib/components/Wheel.svelte";

  let chart = $state<ChartData | null>(null);
  let input = $state<PreviewInput | null>(null);
  let draft = $state("");
  let saving = $state(false);
  let err = $state("");

  const tags = $derived(pinnedTags());
  const names = $derived(
    new Map(chart ? elementsOf(chart).map((e) => [e.tag, e.name]) : []),
  );
  const filedUnder = $derived(tags.map((t) => names.get(t) ?? t).join(" · "));

  onMount(async () => {
    clearPins();
    try {
      const place: PlaceDto = await lastPlace();
      const m = nowMoment();
      const req: PreviewInput = {
        place_id: place.id,
        date: m.date,
        time: m.time,
        house_system: "whole-sign",
        zodiac: "tropical",
        ayanamsa: null,
        lang: "en",
      };
      input = req;
      chart = (await preview(req)).chart;
    } catch (e) {
      err = reason(e);
    }
  });

  async function keep() {
    const words = draft.trim();
    if (!words || !chart || !input) return;
    saving = true;
    err = "";
    try {
      await writeMomentNote(words, tags, anchorFor(input, chart));
      draft = "";
      clearPins();
      notify("note kept");
    } catch (e) {
      err = reason(e);
    } finally {
      saving = false;
    }
  }

  function dismiss() {
    draft = "";
    clearPins();
  }
</script>

<div class="sky" class:drafting={tags.length > 0}>
  {#if chart}
    <!-- interactive: the wheel is an index here, not a display -->
    <div class="wheel"><Wheel {chart} /></div>
  {:else if err}
    <p class="apparatus-text fail">{err}</p>
  {/if}

  <!-- The draft is summoned by the sky, so it is written at its foot rather
       than in a column beside it. Nothing is reserved for it: with nothing
       marked, the screen is only sky. -->
  {#if tags.length > 0}
    <div class="annotation">
      <p class="filed">{filedUnder}</p>
      <!-- svelte-ignore a11y_autofocus -->
      <textarea
        bind:value={draft}
        rows="2"
        autofocus
        placeholder="what you see here"
        aria-label="a note on {filedUnder}"
        onkeydown={(e) => {
          if (e.key === "Escape") dismiss();
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) keep();
        }}
      ></textarea>
      <p class="acts">
        {#if err}<span class="fail">{err}</span>{/if}
        <button type="button" class="ghost" onclick={dismiss}>dismiss</button>
        <button
          type="button"
          class="frame-btn"
          disabled={saving || draft.trim() === ""}
          onclick={keep}>{saving ? "keeping…" : "keep"}</button
        >
      </p>
    </div>
  {/if}
</div>

<style>
  .sky {
    min-height: 100vh;
    display: grid;
    grid-template-rows: 1fr auto;
    justify-items: center;
    padding: clamp(1rem, 3vh, 2.4rem) 1.4rem 4.6rem;
  }
  /* No frame, no outline, no vignette. The wheel is the only drawn thing on
     the screen and it is centred on both axes until something is marked. */
  .wheel {
    width: min(84vh, 78vw, 860px);
    align-self: center;
  }
  /* The annotation is not overlaid and nothing is reserved for it, so the
     sky has to yield the room — which is also the right gesture: the chart
     steps back while you write about it. */
  .drafting .wheel {
    width: min(54vh, 60vw, 560px);
  }
  /* The size change snaps rather than tweening: the wheel is a few hundred
     SVG nodes, and animating its width relayouts every one of them on every
     frame. The annotation's own entrance covers the moment, which is what
     the transition was buying anyway. */
  /* Written at the foot of the sky, held by one rule and the measure — the
     page has no enclosures to inherit and does not introduce any. */
  .annotation {
    width: min(100%, 46rem);
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding-top: 0.7rem;
    border-top: 1px solid var(--line);
  }
  .filed {
    margin: 0;
    font-size: 0.78rem;
    font-variant: small-caps;
    letter-spacing: 0.16em;
    color: var(--brass);
  }
  textarea {
    width: 100%;
    background: transparent;
    color: var(--ink);
    border: none;
    border-bottom: 1px solid var(--line);
    font: inherit;
    font-size: 1.02rem;
    line-height: 1.6;
    padding: 0.2rem 0 0.4rem;
    resize: vertical;
  }
  textarea:focus,
  textarea:focus-visible {
    outline: none;
    border-bottom-color: var(--brass);
  }
  .acts {
    display: flex;
    align-items: baseline;
    justify-content: flex-end;
    gap: 0.8rem;
    margin: 0;
  }
  .fail {
    color: var(--oxblood);
    font-style: italic;
    font-size: 0.86rem;
    margin-right: auto;
  }
  @media (prefers-reduced-motion: no-preference) {
    .annotation {
      animation: rise var(--dur-base) var(--ease-out-expo);
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(5px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
