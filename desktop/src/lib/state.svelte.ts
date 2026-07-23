// The app's shared state (Svelte 5 runes) — one Model, Elm-ish.

import { SvelteSet } from "svelte/reactivity";
import { listAyanamsas, listHouseSystems, listLocales } from "./api";
import type { ChartData, Excerpt, LocaleDto, OptionDto } from "./types";

/** What the app is doing: idle, a synchronous chart build, or transcription at
 * a whole-percent progress. One discriminated shape so consumers ask `isBusy()`
 * and read `.pct` only in the transcribe case — no `typeof`/`!== false` probing. */
export type Busy =
  | { kind: "idle" }
  | { kind: "compute" }
  | { kind: "transcribe"; pct: number };

export const app = $state({
  chart: null as ChartData | null,
  mode: "any" as "any" | "all",
  busy: { kind: "idle" } as Busy,
  /** The form's whisper-model path; a non-empty value enables live recording. */
  model: "",
  /** Whether the Index of Elements legend is expanded. Folded by default so the
   * orrery leads; opening it reveals the full keyboard-accessible mirror. */
  indexOpen: false,
  /** The tag under the pointer/keyboard focus anywhere in the reading — a
   * transient preview that lights the orrery and previews the commentary,
   * distinct from the pinned `selected` set. Null when nothing is focused. */
  hovered: null as string | null,
});

export const selected = new SvelteSet<string>();

/** Whether any operation is in flight (a build or a transcription). */
export function isBusy(): boolean {
  return app.busy.kind !== "idle";
}

/** Reading languages offered in the UI, fetched once from the backend
 * (`list_locales`, sourced from `i18n::Locale`): endonym labels for the
 * selectors and the house-name suffix to strip. Empty until `loadLocales`. */
export const locales = $state<LocaleDto[]>([]);

/** Populate `locales` from the backend once; a no-op after the first call. */
export async function loadLocales() {
  if (locales.length > 0) return;
  try {
    locales.push(...(await listLocales()));
  } catch (e) {
    notify(`${e}`, "error");
  }
}

/** The word to strip from a house name for `code` (" House", " дом"), or ""
 * when the locale list hasn't loaded or the code is unknown. The one client
 * home for the mapping — the strings themselves come from the backend. */
export function houseSuffix(code: string): string {
  return locales.find((l) => l.code === code)?.houseSuffix ?? "";
}

/** House systems and ayanamsas offered in the form, each `{code,label}`,
 * fetched once from the backend (`list_house_systems` / `list_ayanamsas`,
 * sourced from `chart::systems`). Empty until `loadCalcOptions`. */
export const houseSystems = $state<OptionDto[]>([]);
export const ayanamsas = $state<OptionDto[]>([]);

/** Populate the calculation-option lists once; a no-op after the first call. */
export async function loadCalcOptions() {
  try {
    if (houseSystems.length === 0) houseSystems.push(...(await listHouseSystems()));
    if (ayanamsas.length === 0) ayanamsas.push(...(await listAyanamsas()));
  } catch (e) {
    notify(`${e}`, "error");
  }
}

/** Transient focus, shared by the wheel, the index legend, and any sector: set
 * it on pointer-enter / keyboard-focus, clear it on leave / blur. Drives the
 * orrery's relational illumination, the hub read-out, and the commentary
 * preview — one path, so every surface lights the others. */
export function peek(tag: string) {
  app.hovered = tag;
}
export function unpeek() {
  app.hovered = null;
}

/** Transient status notifications — each auto-dismisses (errors linger
 * longer) and any can be dismissed by a click. Replaces the old persistent
 * footer status line. */
export type Toast = { id: number; message: string; kind: "info" | "error" };
export const toasts = $state<Toast[]>([]);
let nextToastId = 0;

export function notify(message: string, kind: "info" | "error" = "info") {
  const id = nextToastId++;
  toasts.push({ id, message, kind });
  setTimeout(() => dismissToast(id), kind === "error" ? 7000 : 4000);
}

export function dismissToast(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i !== -1) toasts.splice(i, 1);
}

/** The contract's Excerpt::matches semantics, mirrored (as in the artifact):
 * empty tag list shows all; any = intersection, all = superset. Shared by the
 * selection filter and the chart-view hover preview. */
export function excerptsMatching(
  chart: ChartData,
  tags: string[],
  mode: "any" | "all",
): Excerpt[] {
  if (tags.length === 0) return chart.excerpts;
  const has = (ex: Excerpt) => (t: string) => ex.tags.includes(t);
  return chart.excerpts.filter((ex) =>
    mode === "any" ? tags.some(has(ex)) : tags.every(has(ex)),
  );
}

export function visibleExcerpts(chart: ChartData): Excerpt[] {
  return excerptsMatching(chart, [...selected], app.mode);
}

export function toggle(tag: string) {
  if (!selected.delete(tag)) selected.add(tag);
}

/** The single element the read-out, wheel illumination, and commentary preview
 * all follow. A pinned selection LOCKS the focus: hovering no longer flips it,
 * until the pin is cleared or another element is pinned (the most recent pin
 * wins). With nothing pinned, the hovered element drives the live preview. */
export function focusedTag(): string | null {
  if (selected.size > 0) {
    const arr = [...selected];
    return arr[arr.length - 1];
  }
  return app.hovered;
}
