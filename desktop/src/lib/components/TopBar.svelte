<script lang="ts">
  // The app's top bar: what this screen is on the left, what you can do on the
  // right. It replaced the console rail fixed along the foot of every view,
  // which read as a status bar and put the screen's actions furthest from the
  // eye. The preferences gear the layout pins at the top right closes the
  // line, so the bar keeps its right end clear for it.
  import type { Snippet } from "svelte";

  let {
    lead,
    acts,
    away = false,
  }: {
    lead: Snippet;
    acts?: Snippet;
    /** Stepped aside — faded and lifted, out of the tab order — while the
     * screen is given over to something else (the reading being presented).
     * The bar stays mounted so it can come back the way it went. */
    away?: boolean;
  } = $props();

  // At rest the bar is the sky itself — a filled strip would paint a band
  // across the gradient the plate stands on. It takes a ground only once
  // something has scrolled under it.
  let scrolled = $state(false);
</script>

<svelte:window onscroll={() => (scrolled = window.scrollY > 4)} />

<header class="bar" class:scrolled class:away inert={away}>
  <div class="lead">{@render lead()}</div>
  {#if acts}<div class="acts">{@render acts()}</div>{/if}
</header>

<style>
  .bar {
    position: sticky;
    top: 0;
    z-index: var(--z-sticky);
    display: flex;
    align-items: center;
    gap: clamp(1rem, 3vw, 2.4rem);
    height: var(--bar-h);
    padding: 0 76px 0 clamp(1.2rem, 3vw, 2rem);
    font-family: var(--font-sans);
    transition:
      background var(--dur-base) var(--ease-out-quint),
      box-shadow var(--dur-base) var(--ease-out-quint),
      opacity 420ms var(--ease-out-quint),
      transform 720ms var(--ease-out-expo);
  }
  .bar.away {
    opacity: 0;
    transform: translateY(-16px);
    pointer-events: none;
  }
  /* opaque enough to hold the line over a scrolled column; the blur is what
     keeps the words passing beneath from reading through as words */
  .bar.scrolled {
    background: rgba(15, 18, 43, 0.86);
    -webkit-backdrop-filter: blur(14px);
    backdrop-filter: blur(14px);
    box-shadow: 0 1px 0 var(--line);
  }
  @media (prefers-reduced-motion: reduce) {
    .bar {
      transition: none;
    }
  }
  .lead {
    display: flex;
    align-items: center;
    gap: clamp(1rem, 3vw, 2.4rem);
    min-width: 0;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-left: auto;
  }
  @media (max-width: 640px) {
    .bar {
      flex-wrap: wrap;
      height: auto;
      row-gap: 0.5rem;
      padding-top: 0.8rem;
      padding-bottom: 0.6rem;
      padding-right: clamp(1.2rem, 3vw, 2rem);
    }
    /* only the first line shares the gear's corner */
    .lead {
      flex-wrap: wrap;
      row-gap: 0.4rem;
      padding-right: 52px;
    }
    .acts {
      flex-basis: 100%;
      flex-wrap: wrap;
      margin-left: 0;
      min-width: 0;
    }
    .acts > :global(.primary) {
      flex: 1;
    }
  }
</style>
