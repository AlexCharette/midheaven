// The pose tween: one Tween over the whole chart pose, so frame, planets,
// and rings glide on a single clock. `Tween.set` retargets from the current
// value, so a new preview landing mid-flight re-aims smoothly — during a drag
// the short linear tween acts as an interpolator between IPC samples.

import { Tween } from "svelte/motion";
import { quintOut } from "svelte/easing";
import { prefersReducedMotion } from "./motion";
import { lerpPose, type Pose } from "./pose";

/** What moved the moment — picks the motion's duration and easing. */
export type MoveSource = "drag" | "keyboard" | "field" | "stepper" | "now";

export const durFor = (s: MoveSource): number =>
  prefersReducedMotion() ? 0
  : s === "drag" ? 140 // --dur-fast: smooths discrete IPC samples into glide
  : s === "keyboard" ? 240 // --dur-base
  : 620; // --dur-slow: field / stepper / set-to-now jumps

// quintOut everywhere — even for drag: re-aimed once per IPC sample it acts
// as exponential smoothing toward the pointer, where a linear chase restarts
// with visible velocity corners (jitter).
export const easeFor = (_s: MoveSource) => quintOut;

export function createChartPose(initial: Pose) {
  const tween = new Tween<Pose>(initial, { interpolate: lerpPose });
  return {
    get current(): Pose {
      return tween.current;
    },
    retarget(target: Pose, source: MoveSource) {
      void tween.set(target, { duration: durFor(source), easing: easeFor(source) });
    },
    snap(target: Pose) {
      void tween.set(target, { duration: 0 });
    },
  };
}

export type ChartPose = ReturnType<typeof createChartPose>;
