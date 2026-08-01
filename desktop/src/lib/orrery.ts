// Where things sit on the orrery — the desktop wheel's own placement rules.
//
// `generated/plate.ts` states the plate every rendition draws: its radii, its
// graduation classes, its de-crowding policy. This module is the orrery's
// *behaviour* over that plate, including the two places it deliberately departs
// from the specification, and it is separate from the component for the usual
// reason: a `$derived` inside `Wheel.svelte` is not reachable from a test.
//
// What stays in the component is projection — turning a longitude and a radius
// into an x and a y, and painting. What is here takes numbers and returns
// numbers.

import { PLATE } from "./generated/plate";
import type { ChartData } from "./types";
import { catOf, planetById } from "./types";

/** The shortest angular distance between two longitudes, in degrees — mirrors
 * Rust's `chart::separation`. Two bodies at 359° and 2° are three degrees
 * apart, not 357. */
export function angularGap(a: number, b: number): number {
  const d = Math.abs(a - b) % 360;
  return Math.min(d, 360 - d);
}

/** Where a body's glyph sits, walking the bodies in longitude order.
 *
 * When two bodies are too close to read side by side the later one steps
 * inward, and the step cascades — three bodies in a degree stack one behind
 * another. Returns a radius per body, in the order given.
 *
 * DEPARTURE from the plate: the specification steps at its threshold; the
 * orrery ramps in over the three degrees above it. A body closing on another
 * during a live scrub slides inward rather than popping. A static chart renders
 * identically — the ramp differs only in the band the paper and artifact
 * renditions never see, because they never see a chart move.
 */
export function crowdedRadii(longitudes: number[]): number[] {
  const c = PLATE.crowding;
  const ramp = 3;
  let previous: { lon: number; r: number } | null = null;
  return longitudes.map((lon) => {
    const gap = previous === null ? Infinity : angularGap(lon, previous.lon);
    const f = Math.max(0, Math.min(1, (c.thresholdDeg + ramp - gap) / ramp));
    const r = f > 0 ? Math.max(previous!.r - c.step * f, c.floor) : PLATE.radii.planet;
    previous = { lon, r };
    return r;
  });
}

/** How far a sign's density bar grows, for a passage weight against the busiest
 * sign's.
 *
 * Square-rooted: a sign with four times the passages reads as twice the bar, so
 * one talkative placement does not flatten the rest of the ring. */
export function densityLength(weight: number, busiest: number, span: number): number {
  if (weight <= 0) return 0;
  return Math.sqrt(weight / Math.max(1, busiest)) * span;
}

/** The longitude the selector pointer aims at for a focused element, or null
 * when the element is not on this chart.
 *
 * Each category answers differently: a body is where it stands, a sign or a
 * house is its midpoint, and an aspect is the bisector of the shorter arc
 * between its two bodies — which is signed arithmetic, so it is stated here
 * rather than inlined at the one place that needs it. */
export function focusLongitude(chart: ChartData, tag: string): number | null {
  const lonOf = (id: string) => planetById(chart, id)?.lon ?? null;
  switch (catOf(tag)) {
    case "planet":
      return lonOf(tag);
    case "sign": {
      const i = chart.signs.findIndex((s) => s.id === tag);
      return i < 0 ? null : i * 30 + 15;
    }
    case "house": {
      const n = Number(tag.split(":")[1]);
      const cusp = chart.houseCusps[n - 1];
      if (cusp === undefined) return null;
      return cusp + chart.houseSweeps[n - 1] / 2;
    }
    case "aspect": {
      const a = chart.aspects.find((x) => x.id === tag);
      if (!a) return null;
      const from = lonOf(a.a);
      const to = lonOf(a.b);
      if (from === null || to === null) return null;
      // the signed shortest arc, halved
      return from + ((((to - from + 540) % 360) - 180) / 2);
    }
    default:
      return null;
  }
}

/** The four chart angles and their labels. DC and IC are not data — they are
 * the opposites of AC and MC. */
export function axisLongitudes(chart: ChartData): { lon: number; label: string }[] {
  return [
    { lon: chart.axes.asc, label: "AC" },
    { lon: chart.axes.mc, label: "MC" },
    { lon: chart.axes.asc + 180, label: "DC" },
    { lon: chart.axes.mc + 180, label: "IC" },
  ];
}
