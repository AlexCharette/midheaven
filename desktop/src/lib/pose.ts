// The chart's animatable pose — every angular quantity the live calculator
// interpolates as one unit (one clock, one easing: houses must never shear
// against planets). Pure math here; the Tween wrapper lives in
// `chartPose.svelte.ts`.

import type { ChartData } from "./types";
import { ringAngles } from "./civil";
import { norm360, positionOf, sweepOf } from "./derive";

export type Pose = {
  asc: number;
  mc: number;
  /** 12 house-cusp longitudes, degrees. */
  cusps: number[];
  /** Planet longitudes by id (`planet:sun`, … including `planet:ascendant`). */
  lons: Record<string, number>;
  /** The two instrument rings, degrees clockwise of the fixed index. */
  timeAngle: number;
  dateAngle: number;
};

/** Shortest-arc lerp in degrees. The result may leave [0,360) — consumers are
 * trigonometric or re-lerped, so that's fine — which is also why pose values
 * must NEVER be compared with `===` against a target (equal only mod 360). */
export const angleLerp = (a: number, b: number, t: number): number =>
  a + ((((b - a) % 360) + 540) % 360 - 180) * t;

/** Quantize to 0.02° — imperceptible, but settled frames then produce
 * identical attribute strings and Svelte skips the DOM writes. */
export const quant = (x: number): number => Math.round(x * 50) / 50;

/** Whole-pose interpolator for the tween: shortest-arc per quantity, exact
 * target at t=1. Quantities the start pose lacks (a body appearing) arrive
 * already in place rather than sweeping in from 0°. */
export const lerpPose =
  (a: Pose, b: Pose) =>
  (t: number): Pose => {
    if (t >= 1) return b;
    const lon = (from: number | undefined, to: number) =>
      quant(from === undefined ? to : angleLerp(from, to, t));
    return {
      asc: lon(a.asc, b.asc),
      mc: lon(a.mc, b.mc),
      cusps: b.cusps.map((c, i) => lon(a.cusps[i], c)),
      lons: Object.fromEntries(
        Object.entries(b.lons).map(([id, l]) => [id, lon(a.lons[id], l)]),
      ),
      timeAngle: lon(a.timeAngle, b.timeAngle),
      dateAngle: lon(a.dateAngle, b.dateAngle),
    };
  };

/** A chart's pose at a civil instant — the single conversion point between
 * the backend's ChartData and the animated presentation. */
export function poseOf(chart: ChartData, minutes: number): Pose {
  const { timeAngle, dateAngle } = ringAngles(minutes);
  return {
    asc: chart.axes.asc,
    mc: chart.axes.mc,
    cusps: [...chart.houseCusps],
    lons: Object.fromEntries(chart.planets.map((p) => [p.id, p.lon])),
    timeAngle,
    dateAngle,
  };
}

/** The other direction: a chart to *render*, made of `target`'s identity and
 * `pose`'s angles — planets, frame and cusps mid-glide; aspects, houses and
 * labels discrete from the target.
 *
 * The derived fields ride on the chart, so they belong to the target's
 * longitudes and are re-derived here for the tweened ones. Everything
 * downstream then reads fields and needs no knowledge of the glide. This lived
 * in the calculator view, which made eleven lines of chart arithmetic a
 * component's business and left it untestable. */
export function project(target: ChartData, pose: Pose): ChartData {
  const cusps = target.houseCusps.map((c, i) => norm360(pose.cusps[i] ?? c));
  return {
    ...target,
    axes: { asc: norm360(pose.asc), mc: norm360(pose.mc) },
    houseCusps: cusps,
    houseSweeps: cusps.map((c, i) => sweepOf(c, cusps[(i + 1) % cusps.length])),
    planets: target.planets.map((p) => {
      const lon = norm360(pose.lons[p.id] ?? p.lon);
      return { ...p, lon, ...positionOf(lon) };
    }),
  };
}
