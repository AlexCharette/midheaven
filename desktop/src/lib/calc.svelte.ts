// The live calculator's model — deliberately separate from the reading session:
// `session.hasReading()` keeps meaning "a reading is open", so the reading view
// and leaving it are untouched. Module state survives entering and leaving
// readings; returning to the calculator finds the explored moment intact.

import { preview } from "./api";
import type { ChartData, PlaceDto } from "./types";
import { fromMinutes, ringAngles } from "./civil";
import { createChartPose, type MoveSource } from "./chartPose.svelte";
import { poseOf } from "./pose";

export const calc = $state({
  /** The draft civil instant (minutes, see `civil.ts`) — the single source of
   * truth every input writes and every projection (rings, fields, previews)
   * reads. 0 = uninitialized; `initCalc` seeds it with "now". */
  minutes: 0,
  place: null as PlaceDto | null,
  houseSystem: "whole-sign",
  zodiac: "tropical",
  ayanamsa: "lahiri",
  /** Locale for element labels, from preferences at startup. */
  lang: "en",
  /** The newest successfully computed chart — the tween's TARGET; the wheel
   * renders the pose-interpolated projection of it. Never read by the
   * reading view. */
  target: null as ChartData | null,
  pending: false,
  /** An invalid moment (Feb 30 typed, DST gap): shown as a quiet caption;
   * `target` keeps the last good chart so the wheel never blanks. */
  error: null as string | null,
  /** Non-fatal notes for the current moment (DST fold). */
  warnings: [] as string[],
  /** The birth-form fly-out. */
  birthOpen: false,
  /** A ring is being held. The wheel hides its derived apparatus (house
   * wedges, aspect chords) for the length of the gesture — whole-sign cusps
   * snap 30° at every sign crossing and aspects pop in/out of orb, which
   * reads as jitter at drag rates — and eases it back in on release. */
  scrubbing: false,
});

/** The displayed pose — retargeted on every preview arrival. */
export const pose = createChartPose({
  asc: 0,
  mc: 90,
  cusps: Array.from({ length: 12 }, (_, i) => i * 30),
  lons: {},
  timeAngle: 0,
  dateAngle: 0,
});

/** The one gate every input writes through (rings, fields, stepper, keyboard,
 * "set to now"). No-ops when the quantized instant is unchanged, which kills
 * ring↔field echo loops structurally. */
export function setDraft(min: number, source: MoveSource) {
  const q = Math.round(min);
  if (q === calc.minutes) return;
  calc.minutes = q;
  requestPreview(source);
}

/** A ring was grabbed: the wheel hides houses/aspects for the gesture. */
export function beginScrub() {
  calc.scrubbing = true;
}

/** The ring was released: the hidden apparatus eases back in. */
export function endScrub() {
  calc.scrubbing = false;
}

// Trailing-edge coalescing: at most one preview in flight; if the draft moves
// meanwhile, exactly one more request fires when it lands. `seq` guards a slow
// stale response from overwriting a newer one (PlacePicker's counter pattern).
let seq = 0;
let inFlight = false;
let queued: MoveSource | null = null;
let started = false;

export function requestPreview(source: MoveSource) {
  if (!calc.place || calc.minutes === 0) return;
  if (inFlight) {
    queued = source;
    return;
  }
  void pump(source);
}

async function pump(source: MoveSource) {
  inFlight = true;
  calc.pending = true;
  let src: MoveSource | null = source;
  while (src !== null) {
    const my = ++seq;
    const snapshot = calc.minutes;
    const m = fromMinutes(snapshot);
    try {
      const res = await preview({
        date: m.date,
        time: m.time,
        place_id: calc.place!.id,
        lang: calc.lang,
        house_system: calc.houseSystem,
        zodiac: calc.zodiac,
        ayanamsa: calc.zodiac === "sidereal" ? calc.ayanamsa : null,
      });
      if (my === seq) {
        calc.target = res.chart;
        calc.error = null;
        calc.warnings = res.warnings;
        const target = poseOf(res.chart, snapshot);
        // The first chart appears in place (the wheel's entrance choreography
        // is the arrival); everything after glides.
        if (started) pose.retarget(target, src);
        else pose.snap(target);
        started = true;
      }
    } catch (e) {
      if (my === seq) {
        // Keep the last good chart on the plate; the rings still glide to the
        // drafted moment (it's a real wall-clock value, just not computable —
        // e.g. inside a DST gap at this place).
        calc.error = String(e);
        const { timeAngle, dateAngle } = ringAngles(snapshot);
        pose.retarget({ ...pose.current, timeAngle, dateAngle }, src);
      }
    }
    src = queued;
    queued = null;
  }
  calc.pending = false;
  inFlight = false;
}
