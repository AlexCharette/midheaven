// The reactive focus — a thin rune wrapper over `focus.ts`.
//
// All of the reasoning lives in `focus.ts`, which is rune-free and therefore
// directly testable; this file only holds the one `$state` and applies the pure
// transitions to it. The same split as `session.ts`/`session.svelte.ts`.
//
// Readers are functions rather than fields because reading a rune inside a
// function still tracks the dependency in a template or `$derived`.
//
// Nothing outside this file writes the focus. Its predecessor exposed a mutable
// `app` object and a `SvelteSet`, so ten files reached in and set fields — which
// is why the hover-preview rule ended up in `+page.svelte` rather than beside
// the pin rule it completes.

import * as rules from "./focus";
import type { Focus, Mode } from "./focus";
import type { ChartData, Excerpt } from "./types";

const store = $state({ value: rules.NOTHING as Focus });

// ---- reads ----

/** The focused element: the most recent pin, else whatever is hovered. */
export const focusedTag = (): string | null => rules.focusedTag(store.value);

export const isPinned = (tag: string): boolean => rules.isPinned(store.value, tag);
export const pinCount = (): number => store.value.pinned.length;
export const hoveredTag = (): string | null => store.value.hovered;
export const mode = (): Mode => store.value.mode;

/** The passages the commentary shows, pins and hover preview both accounted
 * for. */
export const visibleExcerpts = (chart: ChartData): Excerpt[] =>
  rules.visibleExcerpts(chart, store.value);

/** Whether a passage is part of the pinned selection. */
export const touchesPins = (ex: Excerpt): boolean => rules.touchesPins(store.value, ex);

// ---- writes ----

/** Pin or unpin an element. Re-pinning takes the focus. */
export function toggle(tag: string) {
  store.value = rules.toggled(store.value, tag);
}

export function clearPins() {
  store.value = rules.cleared(store.value);
}

/** Transient focus, shared by the wheel, the index legend, and any sector: set
 * it on pointer-enter / keyboard-focus, clear it on leave / blur. Drives the
 * orrery's relational illumination, the hub read-out, and the commentary
 * preview — one path, so every surface lights the others. */
export function peek(tag: string) {
  store.value = rules.peeked(store.value, tag);
}

export function unpeek() {
  store.value = rules.unpeeked(store.value);
}

export function setMode(m: Mode) {
  store.value = rules.withMode(store.value, m);
}

/** Drop pins and hover together — leaving a reading, or opening another. One
 * call, because the two used to be reset side by side at every such site and a
 * missed half left the next reading lit up. */
export function resetFocus() {
  store.value = rules.NOTHING;
}
