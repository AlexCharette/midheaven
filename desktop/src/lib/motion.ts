// Shared motion helpers — one home for "does the viewer want reduced motion?"
// and the keyed-swap transition duration, instead of re-deriving both in each
// component. `SWAP_MS` is intentionally a touch quicker than the CSS
// `--dur-base` token (240ms); change it here to retune every JS swap at once.

/** Whether the OS/browser asks for reduced motion. Safe during SSR (false). */
export function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** Duration (ms) for keyed fade/swap transitions — 0 under reduced motion. */
export const SWAP_MS = 200;
export const swapDuration = (): number => (prefersReducedMotion() ? 0 : SWAP_MS);
