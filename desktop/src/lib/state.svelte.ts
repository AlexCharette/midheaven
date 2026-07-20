// The app's shared state (Svelte 5 runes) — one Model, Elm-ish.

import { SvelteSet } from "svelte/reactivity";
import type { ChartData, Excerpt } from "./types";

export const app = $state({
  chart: null as ChartData | null,
  mode: "any" as "any" | "all",
  status: "",
  /** false = idle · "compute" = fast build · number = transcription percent. */
  busy: false as false | "compute" | number,
  /** The form's whisper-model path; a non-empty value enables live recording. */
  model: "",
});

export const selected = new SvelteSet<string>();

/** The contract's Excerpt::matches semantics, mirrored (as in the artifact):
 * empty selection shows all; any = intersection, all = superset. */
export function visibleExcerpts(chart: ChartData): Excerpt[] {
  const sel = [...selected];
  if (sel.length === 0) return chart.excerpts;
  const has = (ex: Excerpt) => (t: string) => ex.tags.includes(t);
  return chart.excerpts.filter((ex) =>
    app.mode === "any" ? sel.some(has(ex)) : sel.every(has(ex)),
  );
}

export function toggle(tag: string) {
  if (!selected.delete(tag)) selected.add(tag);
}
