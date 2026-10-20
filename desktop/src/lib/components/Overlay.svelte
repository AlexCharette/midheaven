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
    title,
    variant = "plate",
    children,
  }: {
    onclose: () => void;
    label: string;
    /** The sheet's visible head. With one, the overlay draws the head and its
     * close control itself, so every sheet opens the same way and none has to
     * end in a "← back" line to be leavable. */
    title?: string;
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
    {#if title}
      <header class="sheet-head">
        <h2>{title}</h2>
        <button type="button" class="btn bare icon" aria-label="close" title="close (Esc)" onclick={onclose}>
          <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8" /></svg>
        </button>
      </header>
    {/if}
    <div class="sheet-body">
      {@render children()}
    </div>
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
    background: rgba(9, 11, 28, 0.62);
    -webkit-backdrop-filter: blur(3px);
    backdrop-filter: blur(3px);
    border: none;
    cursor: default;
  }
  /* A sheet lifted off the sky: one step lighter than the ground, rounded,
     and held up by a soft shadow rather than a drawn frame. */
  .surface {
    position: relative;
    display: flex;
    flex-direction: column;
    outline: none;
    background: var(--surface-raised);
    border-radius: 12px;
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 24px 64px -16px rgba(0, 0, 0, 0.7),
      0 2px 8px rgba(0, 0, 0, 0.3);
    max-height: min(88vh, 100%);
    width: min(40rem, 94vw);
    overflow: hidden;
  }
  .sheet-head {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem 1rem 0.4rem 1.5rem;
  }
  .sheet-head h2 {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 1.2rem;
    font-weight: 400;
    color: var(--ink);
  }
  .sheet-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.6rem 1.5rem 1.5rem;
  }
  /* the fly-out: a full-height sheet hinged on the right edge */
  .panel .surface {
    width: min(32rem, 94vw);
    max-height: none;
    height: 100%;
    border-radius: 0;
    box-shadow:
      inset 1px 0 0 var(--line),
      -24px 0 64px -16px rgba(0, 0, 0, 0.7);
  }
  .panel .sheet-head {
    padding-top: 1.1rem;
  }
</style>
