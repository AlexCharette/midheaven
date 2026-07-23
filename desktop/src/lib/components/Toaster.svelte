<script lang="ts">
  import { toasts, dismissToast } from "$lib/state.svelte";

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
  /* Above the fixed tools footer; a document-scale stack that never
     intercepts clicks except on a toast itself. */
  .toaster {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 2.6rem;
    z-index: var(--z-toast);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    padding: 0 1rem;
    pointer-events: none;
  }
  /* An engraved plate: hairline frame on the deep field, lifted just off the
     sky. No glow, no gradient — elevation only. */
  .toast {
    pointer-events: auto;
    display: inline-flex;
    align-items: baseline;
    gap: 0.5rem;
    max-width: min(90vw, 34rem);
    padding: 0.4rem 1.1rem;
    background: var(--bg-deep);
    border: 1px solid var(--hairline);
    color: var(--ink);
    font-size: 0.88rem;
    text-align: left;
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.45);
    animation: toast-in 220ms var(--ease-out-expo);
  }
  .toast:hover {
    border-color: var(--ink-3);
  }
  .msg {
    line-height: 1.4;
  }
  /* Colour is never the only signal — the ✗ carries the error reading; the
     message text stays high-contrast ink. */
  .toast.error .mark {
    color: var(--oxblood);
    font-weight: 600;
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .toast {
      animation: none;
    }
  }
</style>
