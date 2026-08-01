// What the reading is pointing at, and everything that follows from it.
//
// Rune-free and pure, so it is directly testable; `focus.svelte.ts` holds the
// one `$state` and adapts. The same split as `session.ts`/`session.svelte.ts`
// and `pose.ts`/`chartPose.svelte.ts`.
//
// These rules used to be scattered: the pin-locks-hover rule lived here, the
// hover-preview half of it in `+page.svelte`, and the tag-matching rule was
// restated four times — once as the canonical `excerptsMatching`, once in
// `+page.svelte` for the hover preview, once in `ChartCore` for the hub count,
// and once in `Commentary` as an inline `.some(t => selected.has(t))`. Each
// restatement was the same rule spelled differently, so a change to it had four
// places to reach.

import type { ChartData, Excerpt } from "./types";
import { catOf, planetById, signOf } from "./types";

/** How a multi-tag selection filters: `any` = the passage touches one of the
 * tags, `all` = it touches every one. */
export type Mode = "any" | "all";

/** What the reading is pointing at.
 *
 * `pinned` is ordered — the most recent pin wins the focus — and is a list
 * rather than a set precisely so that order is part of the value.
 * `hovered` is the transient pointer/keyboard focus. */
export type Focus = {
  pinned: string[];
  hovered: string | null;
  mode: Mode;
};

export const NOTHING: Focus = { pinned: [], hovered: null, mode: "any" };

// ---- the focus itself ----

/** The single element the read-out, the wheel's illumination and the commentary
 * preview all follow.
 *
 * A pin LOCKS the focus: hovering no longer flips it, until the pin is cleared
 * or another element is pinned (the most recent wins). With nothing pinned, the
 * hovered element drives the live preview. */
export function focusedTag(f: Focus): string | null {
  if (f.pinned.length > 0) return f.pinned[f.pinned.length - 1];
  return f.hovered;
}

export const isPinned = (f: Focus, tag: string) => f.pinned.includes(tag);

/** Pin an unpinned tag, unpin a pinned one. Re-pinning moves it to the end, so
 * it takes the focus. */
export function toggled(f: Focus, tag: string): Focus {
  const pinned = f.pinned.includes(tag)
    ? f.pinned.filter((t) => t !== tag)
    : [...f.pinned, tag];
  return { ...f, pinned };
}

export const cleared = (f: Focus): Focus => ({ ...f, pinned: [] });
export const peeked = (f: Focus, tag: string): Focus => ({ ...f, hovered: tag });
export const unpeeked = (f: Focus): Focus => ({ ...f, hovered: null });
export const withMode = (f: Focus, mode: Mode): Focus => ({ ...f, mode });

// ---- what the focus selects ----

/** An empty tag list shows everything; `any` = the passage touches one of the
 * tags, `all` = it touches every one.
 *
 * The one home for this rule in the app. The emitted artifact states it a second
 * time (`matches` in `templates/reading.html`) because it is a standalone file
 * that cannot import this one — that copy is the only other, and it says so.
 * Rust used to carry a third in `contract.rs`, documented as the shared one and
 * called by nobody. */
export function matching(chart: ChartData, tags: string[], mode: Mode): Excerpt[] {
  if (tags.length === 0) return chart.excerpts;
  const has = (ex: Excerpt) => (t: string) => ex.tags.includes(t);
  return chart.excerpts.filter((ex) =>
    mode === "any" ? tags.some(has(ex)) : tags.every(has(ex)),
  );
}

/** The passages the commentary shows.
 *
 * With nothing pinned, hovering an element previews just its passages; once
 * anything is pinned the hover preview stops and the list tracks the pinned
 * selection (an empty selection being the whole reading). */
export function visibleExcerpts(chart: ChartData, f: Focus): Excerpt[] {
  if (f.pinned.length === 0 && f.hovered !== null) {
    return matching(chart, [f.hovered], "any");
  }
  return matching(chart, f.pinned, f.mode);
}

/** Whether a passage is part of the pinned selection — the wash the commentary
 * puts on its pinned rows. Nothing is washed when nothing is pinned. */
export function touchesPins(f: Focus, ex: Excerpt): boolean {
  return f.pinned.length > 0 && ex.tags.some((t) => f.pinned.includes(t));
}

/** How many passages touch a single tag — the hub read-out's count. */
export const passageCount = (chart: ChartData, tag: string): number =>
  matching(chart, [tag], "any").length;

// ---- what lights up with it ----

/** The elements a given element is bound to, so the orrery can light up every
 * relation at once when one is focused. The set never includes the focus tag
 * itself — that is styled as `.focus`, its relations as `.rel`. Symmetric:
 * focusing a planet lights its sign/house/aspects/partners; focusing any of
 * those lights the planet back.
 *   planet  → its sign, its house, its aspects, the far planet of each aspect
 *   sign    → the planets standing in it
 *   house   → the planets tenanting it
 *   aspect  → its two endpoint planets */
export function relatedTo(chart: ChartData, tag: string): Set<string> {
  const rel = new Set<string>();
  const cat = catOf(tag);
  if (cat === "planet") {
    const p = planetById(chart, tag);
    if (p) {
      rel.add(signOf(chart, p).id);
      rel.add(`house:${p.house}`);
      for (const a of chart.aspects) {
        if (a.a === tag) { rel.add(a.id); rel.add(a.b); }
        else if (a.b === tag) { rel.add(a.id); rel.add(a.a); }
      }
    }
  } else if (cat === "sign") {
    for (const p of chart.planets) if (signOf(chart, p).id === tag) rel.add(p.id);
  } else if (cat === "house") {
    const n = Number(tag.split(":")[1]);
    for (const p of chart.planets) if (p.house === n) rel.add(p.id);
  } else if (cat === "aspect") {
    const a = chart.aspects.find((x) => x.id === tag);
    if (a) { rel.add(a.a); rel.add(a.b); }
  }
  return rel;
}

/** The sign and house tags some body stands in — the occupancy half of the
 * index's relevance rule, and the same relation `relatedTo` walks per element. */
export function occupiedTags(chart: ChartData): Set<string> {
  return new Set(chart.planets.flatMap((p) => [signOf(chart, p).id, `house:${p.house}`]));
}

/** Passage weight per sign (index 0 = Aries … 11 = Pisces), for the outer
 * density track: how much the reading dwelt on each sign. A passage counts
 * toward a sign when it is tagged with that sign, or with a planet standing in
 * it — so a talkative Sun lights up its own sign even when the words never
 * named the sign directly. */
export function signDensity(chart: ChartData): number[] {
  const planetSign = new Map(chart.planets.map((p) => [p.id, p.sign]));
  const weight = new Array(12).fill(0);
  for (const ex of chart.excerpts) {
    const hit = new Set<number>();
    for (const tag of ex.tags) {
      if (tag.startsWith("sign:")) {
        const i = chart.signs.findIndex((s) => s.id === tag);
        if (i !== -1) hit.add(i);
      } else if (planetSign.has(tag)) {
        hit.add(planetSign.get(tag)!);
      }
    }
    for (const i of hit) weight[i]++;
  }
  return weight;
}
