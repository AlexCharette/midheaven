<script lang="ts">
  import "$lib/theme.css";
  import Overlay from "$lib/components/Overlay.svelte";
  import Preferences from "$lib/components/Preferences.svelte";
  import Toaster from "$lib/components/Toaster.svelte";
  import { isRecording } from "$lib/session.svelte";
  import { isPresenting } from "$lib/present.svelte";
  let { children } = $props();

  // Preferences is app-level chrome, not one view's business, so its control
  // and its panel live here. The landing view used to own both, which left an
  // open reading with no route to them at all — you had to abandon the reading.
  let prefsOpen = $state(false);
  let gear = $state<HTMLButtonElement | null>(null);

  // Refused mid-take, for the reason leaving a reading is: the panel's veil
  // covers the footer, and the footer is where the only stop control lives —
  // opening it over a running recorder would strand the take.
  const refusal = $derived(isRecording() ? "finish the take first" : null);

  function close() {
    prefsOpen = false;
    // The overlay took focus on mount; hand it back rather than dropping it on
    // the body, so a keyboard user resumes where they were.
    gear?.focus();
  }
</script>

<!-- `inert` while the panel is open: the control is fixed, so it is the
     document's first tab stop, and without this it stays reachable from behind
     its own veil. Toasts sit outside — they are drawn above the veil on purpose
     (see Overlay), so they must stay legible and dismissible while it is up. -->
<div inert={prefsOpen}>
  <!-- not on a screen being presented: a client has no use for it -->
  {#if !isPresenting()}
    <button
      class="prefs"
      bind:this={gear}
      onclick={() => (prefsOpen = true)}
      disabled={refusal !== null}
      title={refusal ?? "preferences"}
      aria-label="preferences"
      aria-haspopup="dialog"
    >
      <!-- An engraved gear: the standard affordance in the plate's own line-work —
           teeth as radial ticks off a hairline rim, the figure the wheel's
           graduation band already draws. Six teeth, not eight: eight reads as a
           ship's helm at this size. The hub ring is load-bearing, not ornament —
           without it the mark reads as a sun, which in an astrology app is a
           glyph that already means something else.
           20-unit box at 20px, so `stroke-width: 1` is exactly the 1px hairline
           every border in the app is drawn with. -->
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true" focusable="false">
        <circle cx="10" cy="10" r="5.6" />
        <circle cx="10" cy="10" r="2.9" />
        {#each [0, 60, 120, 180, 240, 300] as angle (angle)}
          <line x1="10" y1="1.1" x2="10" y2="4.4" transform="rotate({angle} 10 10)" />
        {/each}
      </svg>
    </button>
  {/if}

  {@render children()}
</div>

<Toaster />

{#if prefsOpen}
  <Overlay label="preferences" title="Preferences" onclose={close}>
    <Preferences onclose={close} />
  </Overlay>
{/if}

<style>
  /* Pinned at the top bar's right end, centred on its 64px line. The app-wide
     plate edge it used to sit inside is gone: the sky needs no enclosure to
     read as the figure. */
  .prefs {
    position: fixed;
    top: 15px;
    right: 20px;
    z-index: var(--z-chrome);
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: var(--radius-sm);
    color: var(--ink-3);
    transition:
      color var(--dur-fast) var(--ease-out-quint),
      background var(--dur-fast) var(--ease-out-quint);
  }
  .prefs svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
    transition: transform var(--dur-fast) var(--ease-out-quint);
  }
  .prefs:hover:not(:disabled) {
    color: var(--ink);
    background: var(--surface-hover);
  }
  /* pressed feedback only — the gear does not spin on hover; that would be
     decoration, and it conveys nothing about state */
  .prefs:active:not(:disabled) svg {
    transform: scale(0.93);
  }
  .prefs:disabled {
    opacity: 0.4;
    cursor: default;
  }
  @media (prefers-reduced-motion: reduce) {
    .prefs,
    .prefs svg {
      transition: none;
    }
  }
</style>
