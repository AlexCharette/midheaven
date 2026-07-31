// The live calculator's reactive state — a thin rune wrapper over `preview.ts`
// and `pose.ts`.
//
// Deliberately separate from the reading session: `session.hasReading()` keeps
// meaning "a reading is open", so the reading view and leaving it are untouched.
// Module state survives entering and leaving readings; returning to the
// calculator finds the explored moment intact.
//
// This replaced a twelve-field mutable object that both of its components wrote
// into directly. The documented "one gate every input writes through" was true
// only of the moment — every other field was assigned from `Calculator.svelte`,
// so a change to one never asked for the preview it invalidated. Here the gate
// is the interface: there are no fields to assign.

import { preview as sendPreview } from "./api";
import type { ChartData, PlaceDto } from "./types";
import { ringAngles } from "./civil";
import { createChartPose, type MoveSource } from "./chartPose.svelte";
import { poseOf, project } from "./pose";
import { createPump, requestFor, type Draft, type Outcome } from "./preview";

const draft = $state<Draft>({
  minutes: 0,
  place: null,
  houseSystem: "whole-sign",
  zodiac: "tropical",
  ayanamsa: "lahiri",
  lang: "en",
});

const result = $state({
  /** The newest successfully computed chart — the tween's TARGET; the wheel
   * renders the pose-interpolated projection of it. Never read by the reading
   * view. */
  target: null as ChartData | null,
  pending: false,
  /** An invalid moment (Feb 30 typed, DST gap): shown as a quiet caption.
   * `target` keeps the last good chart so the wheel never blanks. */
  error: null as string | null,
  /** Non-fatal notes for the current moment (a DST fold). */
  warnings: [] as string[],
  /** A ring is being held. The wheel hides its derived apparatus (house wedges,
   * aspect chords) for the length of the gesture — whole-sign cusps snap 30° at
   * every sign crossing and aspects pop in and out of orb, which reads as jitter
   * at drag rates — and eases it back in on release. */
  scrubbing: false,
});

/** The displayed pose — retargeted on every arrival. */
const pose = createChartPose({
  asc: 0,
  mc: 90,
  cusps: Array.from({ length: 12 }, (_, i) => i * 30),
  lons: {},
  timeAngle: 0,
  dateAngle: 0,
});

const pump = createPump({
  next: () => {
    const input = requestFor(draft);
    return input === null ? null : { input, minutes: draft.minutes };
  },
  send: async (input) => {
    const res = await sendPreview(input);
    return { chart: res.chart, warnings: res.warnings };
  },
  settle: (outcome: Outcome) => {
    if (outcome.ok) {
      result.target = outcome.chart;
      result.error = null;
      result.warnings = outcome.warnings;
      const target = poseOf(outcome.chart, outcome.minutes);
      if (outcome.first) pose.snap(target);
      else pose.retarget(target, outcome.source);
    } else {
      // Keep the last good chart on the plate; the rings still glide to the
      // drafted moment (a real wall-clock value, just not computable there).
      result.error = outcome.reason;
      const { timeAngle, dateAngle } = ringAngles(outcome.minutes);
      pose.retarget({ ...pose.current, timeAngle, dateAngle }, outcome.source);
    }
  },
  pending: (p) => (result.pending = p),
});

// ---- reads ----

/** The drafted civil instant, in minutes. */
export const minutes = (): number => draft.minutes;
export const place = (): PlaceDto | null => draft.place;
export const options = () => ({
  houseSystem: draft.houseSystem,
  zodiac: draft.zodiac,
  ayanamsa: draft.ayanamsa,
  lang: draft.lang,
});
export const isSeeded = (): boolean => draft.minutes !== 0;

export const pending = (): boolean => result.pending;
export const error = (): string | null => result.error;
export const warnings = (): string[] => result.warnings;
export const scrubbing = (): boolean => result.scrubbing;

/** The two instrument-ring angles, mid-glide. */
export const ringPose = () => ({
  timeAngle: pose.current.timeAngle,
  dateAngle: pose.current.dateAngle,
});

/** The chart to render: the target's identity with the pose's angles. Null
 * until the first preview lands. */
export const displayed = (): ChartData | null =>
  result.target === null ? null : project(result.target, pose.current);

// ---- writes ----

/** The one gate every moment input goes through (rings, fields, stepper,
 * keyboard, "set to now"). No-ops when the quantized instant is unchanged, which
 * kills ring↔field echo loops structurally. */
export function setMoment(min: number, source: MoveSource) {
  const q = Math.round(min);
  if (q === draft.minutes) return;
  draft.minutes = q;
  pump.request(source);
}

/** Establish the draft without asking for anything — the calculator's first
 * paint, which seeds place, moment and preferences together and then calls
 * `refresh` once, rather than firing a request per field. */
export function seed(next: Partial<Draft>) {
  Object.assign(draft, next);
}

/** Choose a place. Asks for the preview the change invalidated. */
export function setPlace(p: PlaceDto | null) {
  draft.place = p;
  pump.request("field");
}

/** Change any calculation choice. Asks for the preview the change invalidated —
 * which the caller used to have to remember to do, and one of them didn't: every
 * field but the moment was assigned directly, so nothing re-previewed. */
export function setOptions(next: Partial<Omit<Draft, "minutes" | "place">>) {
  Object.assign(draft, next);
  pump.request("field");
}

/** Ask for a preview of the draft as it stands — the first paint, once the
 * place and moment are seeded. */
export function refresh(source: MoveSource = "now") {
  pump.request(source);
}

/** A ring was grabbed: the wheel hides houses/aspects for the gesture. */
export function beginScrub() {
  result.scrubbing = true;
}

/** The ring was released: the hidden apparatus eases back in. */
export function endScrub() {
  result.scrubbing = false;
}

/** Put the instrument rings at the drafted moment without gliding — the start of
 * a drag, so the ring sits under the pointer rather than chasing it. */
export function snapRings(min: number) {
  pose.snap({ ...pose.current, ...ringAngles(min) });
}
