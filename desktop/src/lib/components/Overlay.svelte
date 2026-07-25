<script lang="ts">
  // The app's one overlay surface: a fixed veil at --z-overlay (above the
  // fixed footer, below toasts — a native <dialog>'s top layer would cover
  // the toasts, so it's a positioned veil instead). Esc and the scrim close;
  // focus moves inside on mount. Two shapes: a centred plate (library,
  // preferences) and a right-edge panel (the birth form's fly-out).
  import { swapDuration } from "$lib/motion";
  import { expoOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import type { Snippet } from "svelte";

  let {
    onclose,
    label,
    variant = "plate",
    children,
  }: {
    onclose: () => void;
    label: string;
    variant?: "plate" | "panel";
    children: Snippet;
  } = $props();

  const dur = swapDuration();

  function mountFocus(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="veil" class:panel={variant === "panel"} transition:fade={{ duration: dur }}>
  <button class="scrim" aria-label="close" tabindex="-1" onclick={onclose}></button>
  <div
    class="surface"
    role="dialog"
    aria-modal="true"
    aria-label={label}
    tabindex="-1"
    use:mountFocus
    transition:fly={{
      x: variant === "panel" ? 48 : 0,
      y: variant === "panel" ? 0 : 8,
      duration: dur ? 240 : 0,
      easing: expoOut,
    }}
  >
    {@render children()}
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: var(--z-overlay);
    display: grid;
    place-items: center;
    padding: 1.2rem;
  }
  .veil.panel {
    place-items: stretch end;
    padding: 0;
  }
  .scrim {
    position: absolute;
    inset: 0;
    background: rgba(13, 16, 38, 0.6);
    border: none;
    cursor: default;
  }
  .surface {
    position: relative;
    outline: none;
    background: var(--bg-deep);
    border: 1px solid var(--hairline);
    max-height: min(92vh, 100%);
    overflow-y: auto;
    width: min(38rem, 94vw);
    padding: 1.2rem 1.5rem;
  }
  /* the fly-out: a full-height leaf hinged on the right edge, wide enough
     for the birth form's label gutter + field grid */
  .panel .surface {
    width: min(36rem, 94vw);
    max-height: none;
    height: 100%;
    border: none;
    border-left: 1px solid var(--hairline);
    padding: 1.4rem 1.6rem 2rem;
  }
</style>
