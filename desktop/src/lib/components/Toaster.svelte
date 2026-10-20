<script lang="ts">
  import { toasts, dismissToast } from "$lib/toasts.svelte";

  // Force text presentation on the error mark, matching the glyph convention.
  const mark = "✗︎"; // ✗
</script>

<div class="toaster" aria-live="polite">
  {#each toasts as t (t.id)}
    <button
      type="button"
      class="toast"
      class:error={t.kind === "error"}
      role={t.kind === "error" ? "alert" : "status"}
      title="dismiss"
      onclick={() => dismissToast(t.id)}
    >
      {#if t.kind === "error"}<span class="mark" aria-hidden="true">{mark}</span>{/if}
      <span class="msg">{t.message}</span>
    </button>
  {/each}
</div>

<style>
  /* The bottom-right corner, clear of the reading column's words: a toast
     confirms an act, and the act happened up in the bar or in the stream.
     Never intercepts clicks except on a toast itself. */
  .toaster {
    position: fixed;
    right: clamp(1rem, 2.5vw, 1.6rem);
    bottom: clamp(1rem, 2.5vw, 1.6rem);
    z-index: var(--z-toast);
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.5rem;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: inline-flex;
    align-items: baseline;
    gap: 0.5rem;
    max-width: min(88vw, 26rem);
    padding: 0.6rem 0.95rem;
    border-radius: var(--radius);
    background: var(--surface-raised);
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 12px 32px -10px rgba(0, 0, 0, 0.65);
    color: var(--ink);
    font-family: var(--font-sans);
    font-size: 0.84rem;
    text-align: left;
    animation: toast-in 260ms var(--ease-out-expo);
  }
  .toast:hover {
    box-shadow:
      inset 0 0 0 1px var(--hairline),
      0 12px 32px -10px rgba(0, 0, 0, 0.65);
  }
  .msg {
    line-height: 1.4;
  }
  /* Colour is never the only signal — the ✗ carries the error reading; the
     message text stays high-contrast ink. */
  .toast.error {
    box-shadow:
      inset 0 0 0 1px rgba(192, 84, 104, 0.45),
      0 12px 32px -10px rgba(0, 0, 0, 0.65);
  }
  .toast.error .mark {
    color: var(--oxblood);
    font-weight: 600;
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .toast {
      animation: none;
    }
  }
</style>
