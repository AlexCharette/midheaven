// The app's shared state (Svelte 5 runes) — one Model, Elm-ish.

import { SvelteSet } from "svelte/reactivity";
import type { ChartData, Excerpt } from "./types";

export const app = $state({
  chart: null as ChartData | null,
  mode: "any" as "any" | "all",
  status: "",
  /** Transcription progress (whole percent) while a build runs; null = idle. */
  progress: null as number | null,
  building: false,
});

export const selected = new SvelteSet<string>();

/** The contract's Excerpt::matches semantics, mirrored (as in the artifact). */
export function matches(ex: Excerpt): boolean {
  if (selected.size === 0) return true;
  const has = (t: string) => ex.tags.includes(t);
  return app.mode === "any" ? [...selected].some(has) : [...selected].every(has);
}

export function toggle(tag: string) {
  if (!selected.delete(tag)) selected.add(tag);
}
