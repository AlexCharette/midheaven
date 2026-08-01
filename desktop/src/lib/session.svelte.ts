// The reactive reading session — a thin rune wrapper over `session.ts`.
//
// All of the reasoning lives in `session.ts`, which is rune-free and therefore
// directly testable; this file only holds the one `$state` and applies
// transitions to it. The same split as `pose.ts` / `chartPose.svelte.ts`.
//
// Readers are functions rather than fields because reading a rune inside a
// function still tracks the dependency in a template or `$derived` — the
// convention `isBusy()` in `busy.svelte.ts` already follows.

import * as machine from "./session";
import type { Session, Transition } from "./session";
import { notify } from "./toasts.svelte";
import type { ChartData } from "./types";

const store = $state({ value: machine.IDLE as Session });

/** Apply a transition, surfacing a refusal as an error toast.
 *
 * Every user-reachable refusal should already be unreachable — the guards below
 * drive the corresponding control's `disabled` state — so a toast here means a
 * control was left ungated. Returns whether the transition applied, for callers
 * that need to branch. */
function apply(t: Transition): boolean {
  if (!t.ok) {
    notify(t.reason, "error");
    return false;
  }
  store.value = t.session;
  return true;
}

// ---- reads ----

/** The open chart, or null when no reading is open. Replaces the former
 * `app.chart`, which doubled as an undocumented proxy for the backend having a
 * session at all. */
export function chart(): ChartData | null {
  return machine.chartOf(store.value);
}

export function hasReading(): boolean {
  return machine.isOpen(store.value);
}

export function isRecording(): boolean {
  return machine.isRecording(store.value);
}

// ---- guards, for `disabled` and its title ----

export function whyCannotLeave(): string | null {
  return machine.whyCannotLeave(store.value);
}

export function whyCannotOpenAnother(): string | null {
  return machine.whyCannotOpenAnother(store.value);
}

export function whyCannotRecalculate(): string | null {
  return machine.whyCannotRecalculate(store.value);
}

export function whyCannotRecord(): string | null {
  return machine.whyCannotRecord(store.value);
}

// ---- transitions ----

/** A reading arrived, from a build or reopened from the library. */
export function openReading(chart: ChartData): boolean {
  return apply(machine.opened(store.value, chart));
}

/** The user left the reading. Refused while recording. */
export function leaveReading(): boolean {
  return apply(machine.closed(store.value));
}

/** The backend confirmed a recorder is running — call after `start_recording`
 * resolves, never before. */
export function takeBegan(): boolean {
  return apply(machine.recordingStarted(store.value));
}

/** Recording ended. Pass the routed chart, or null when transcription failed:
 * the recorder has stopped either way, so the phase leaves `recording` and a
 * failure keeps the chart the session already had. */
export function takeEnded(chart: ChartData | null): boolean {
  return apply(machine.recordingStopped(store.value, chart));
}

/** Apply a recalculated chart. Unlike `updateChart` this is refused mid-take,
 * because a reproject is a round trip and a take can begin while it is out. */
export function applyRecalculation(chart: ChartData): boolean {
  return apply(machine.recalculated(store.value, chart));
}

/** A command returned an updated chart for the reading already open — curation:
 * a merge, an amendment, a filing, a deletion. Keeps the phase, so curating
 * mid-take does not end the take. A completed *recalculation* goes through
 * `applyRecalculation`, which is refused mid-take. */
export function updateChart(chart: ChartData): boolean {
  return apply(machine.chartReplaced(store.value, chart));
}
