// The reading session as a state machine — rune-free on purpose.
//
// A session is the open working state the backend holds: a chart, and possibly
// a live recorder capturing a take into it. Ten of the twenty-five Tauri
// commands require one (`"no chart has been built yet"`), and the frontend used
// to track it as three independent client facts — `app.chart`, plus a
// `recording` boolean and a `recSecs` timer local to the route — with no
// command to close it. Nothing tied them together, so `back()` could clear the
// chart while leaving a recorder running with its stop control unmounted.
//
// Here the two facts that must agree are one value. "Recording without a chart"
// is not a state you can write down, so that class of desync is gone by
// construction rather than by discipline.
//
// This module holds no runes so it can be imported and exercised directly —
// the same split as `pose.ts` (pure, tested) against `chartPose.svelte.ts` (the
// Tween wrapper). `session.svelte.ts` owns the reactive copy and delegates
// every transition here.
//
// `recSecs` deliberately stays with the route: it is a display counter, and
// because every other path refuses while recording (see `whyCannotLeave` and
// friends) the phase cannot change underneath it. If those refusals are ever
// relaxed, the counter has to move here too.

import type { ChartData } from "./types";

/** The session, as the only shape it can take. */
export type Session =
  | { phase: "idle" }
  | { phase: "open"; chart: ChartData }
  | { phase: "recording"; chart: ChartData };

/** A transition either produces the next session or refuses, with the reason
 * the UI shows. Refusals are values rather than thrown errors because the same
 * reason drives a button's `disabled` title before anyone tries the action. */
export type Transition =
  | { ok: true; session: Session }
  | { ok: false; reason: string };

export const IDLE: Session = { phase: "idle" };

/** The reason recording blocks an action — the one wording every refusal uses. */
const WHILE_RECORDING = "stop transcribing first";

// ---- reading the session ----

/** The open chart, or null when idle. The direct replacement for the old
 * `app.chart` read, and the proxy ten commands' precondition rides on. */
export function chartOf(session: Session): ChartData | null {
  return session.phase === "idle" ? null : session.chart;
}

export function isOpen(session: Session): boolean {
  return session.phase !== "idle";
}

export function isRecording(session: Session): boolean {
  return session.phase === "recording";
}

// ---- guards: why an action is unavailable, or null when it is ----

/** Leaving the reading. Refused while recording: the stop control lives inside
 * the reading view, so leaving would stranded a recorder the user can no longer
 * reach, and its take would land on whatever chart is opened next. */
export function whyCannotLeave(session: Session): string | null {
  if (session.phase === "recording") return WHILE_RECORDING;
  return null;
}

/** Opening a different reading — a fresh build, or one from the library.
 * Refused while recording for the same reason: it would re-seat the chart under
 * a live recorder. */
export function whyCannotOpenAnother(session: Session): string | null {
  if (session.phase === "recording") return WHILE_RECORDING;
  return null;
}

/** Recomputing this chart's geometry for a different house system or zodiac.
 * Refused while recording because the backend releases the session across the
 * transcription await, so a recalculation can land in that gap and the take's
 * passages would be filed against a chart the user has already replaced. */
export function whyCannotRecalculate(session: Session): string | null {
  if (session.phase === "recording") return WHILE_RECORDING;
  if (session.phase === "idle") return "no reading is open";
  return null;
}

/** Starting a take. */
export function whyCannotRecord(session: Session): string | null {
  if (session.phase === "recording") return "already transcribing";
  if (session.phase === "idle") return "no reading is open";
  return null;
}

// ---- transitions ----

/** A reading arrived — from a build, or reopened from the library. */
export function opened(session: Session, chart: ChartData): Transition {
  const refused = whyCannotOpenAnother(session);
  if (refused) return { ok: false, reason: refused };
  return { ok: true, session: { phase: "open", chart } };
}

/** The user left the reading. */
export function closed(session: Session): Transition {
  const refused = whyCannotLeave(session);
  if (refused) return { ok: false, reason: refused };
  return { ok: true, session: IDLE };
}

/** The backend confirmed a recorder is running. Called after `start_recording`
 * resolves, never before — a refusal there must leave the phase alone. */
export function recordingStarted(session: Session): Transition {
  if (session.phase !== "open") {
    return { ok: false, reason: whyCannotRecord(session) ?? "cannot transcribe" };
  }
  return { ok: true, session: { phase: "recording", chart: session.chart } };
}

/** Recording ended. `chart` is the routed result, or null when transcription
 * failed — the recorder has stopped either way, so the phase leaves `recording`
 * in both cases and a failure simply keeps the chart it had. */
export function recordingStopped(session: Session, chart: ChartData | null): Transition {
  if (session.phase !== "recording") {
    return { ok: false, reason: "not transcribing" };
  }
  return { ok: true, session: { phase: "open", chart: chart ?? session.chart } };
}

/** A command returned an updated chart for the session already open — curation
 * (merge, amend, file, delete) or a completed recalculation. Keeps the phase, so
 * curating during a take does not end it; guard recalculation with
 * `whyCannotRecalculate` before calling. */
export function chartReplaced(session: Session, chart: ChartData): Transition {
  if (session.phase === "idle") {
    return { ok: false, reason: "no reading is open" };
  }
  return { ok: true, session: { ...session, chart } };
}
