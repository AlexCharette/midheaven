<script lang="ts">
  // ─── THROWAWAY SPIKE ───────────────────────────────────────────────────
  // Two screens built to be looked at and judged, not kept: a chart with no
  // boundary at all, and a ledger of notes. Nothing in the app depends on
  // this directory — deleting `src/routes/spike/` and the one link in
  // Calculator's rail removes it completely.
  //
  // The app draws a hairline frame with corner ticks around every surface
  // (`+layout.svelte`). A screen whose whole premise is "no boundary" cannot
  // sit inside one, so this layout suppresses it. That suppression is itself
  // part of what is being judged: whether the app still reads as engraved
  // once nothing is enclosing it.
  import { page } from "$app/state";
  let { children } = $props();

  const here = $derived(page.url.pathname);
</script>

<svelte:head>
  <style>
    .app-frame {
      display: none;
    }
  </style>
</svelte:head>

{@render children()}

<nav class="spike-rail" aria-label="spike">
  <span class="mark apparatus-text">spike</span>
  <a href="/spike/chart" aria-current={here.endsWith("/chart") ? "page" : undefined}>the chart</a>
  <a href="/spike/ledger" aria-current={here.endsWith("/ledger") ? "page" : undefined}>the ledger</a>
  <a class="out" href="/">← back to the app</a>
</nav>

<style>
  .spike-rail {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: var(--z-footer);
    display: flex;
    align-items: baseline;
    gap: 1.3rem;
    padding: 0.5rem clamp(1.2rem, 3vw, 2.6rem);
    background: var(--bg-deep);
    border-top: 1px solid var(--line);
    font-size: 0.86rem;
  }
  .mark {
    font-variant: small-caps;
    letter-spacing: 0.18em;
    font-style: normal;
    color: var(--oxblood);
  }
  a {
    color: var(--ink-3);
    text-decoration: none;
    font-style: italic;
    border-bottom: 1px solid transparent;
  }
  a:hover {
    color: var(--ink);
  }
  a[aria-current="page"] {
    color: var(--ink);
    border-bottom-color: var(--brass);
    font-style: normal;
    font-variant: small-caps;
    letter-spacing: 0.1em;
  }
  .out {
    margin-left: auto;
  }
  @media (max-width: 560px) {
    .spike-rail {
      gap: 0.9rem;
      font-size: 0.78rem;
      white-space: nowrap;
    }
    .mark {
      display: none;
    }
  }
  @media (prefers-reduced-motion: no-preference) {
    a {
      transition: color var(--dur-fast) var(--ease-out-quint);
    }
  }
</style>
