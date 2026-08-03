// The live calculator's request pump: how a moving draft moment becomes a
// stream of computed charts without flooding the backend.
//
// Rune-free and transport-injected, so it is directly testable. Its logic used
// to sit in four module-level flags (`seq`, `inFlight`, `queued`, `started`)
// that nothing outside the module could reach and no test could observe, next to
// the state it wrote — which is why the hardest part of the calculator had no
// coverage at all.

import type { ChartData, PlaceDto, PreviewInput } from "./types";
import type { MoveSource } from "./chartPose.svelte";
import { fromMinutes } from "./civil";
import { reason } from "./failure";
import { SIDEREAL } from "./types";

/** Everything a preview is computed from. The place is nullable because the
 * calculator opens before one is resolved. */
export type Draft = {
  /** The civil instant, in minutes (see `civil.ts`). 0 = not seeded yet. */
  minutes: number;
  place: PlaceDto | null;
  houseSystem: string;
  zodiac: string;
  ayanamsa: string;
  lang: string;
};

/** The request for a draft, or `null` when there is nothing to ask for yet —
 * no place resolved, or no moment seeded. The one place that decides what a
 * preview asks for, and the one that knows sidereal is what makes the ayanamsa
 * relevant. */
export function requestFor(draft: Draft): PreviewInput | null {
  if (!draft.place || draft.minutes === 0) return null;
  const m = fromMinutes(draft.minutes);
  return {
    date: m.date,
    time: m.time,
    place_id: draft.place.id,
    lang: draft.lang,
    house_system: draft.houseSystem,
    zodiac: draft.zodiac,
    ayanamsa: draft.zodiac === SIDEREAL ? draft.ayanamsa : null,
  };
}

/** What one round of the pump produced.
 *
 * A failure is not an error state: a drafted moment can be perfectly real and
 * still not computable at its place (inside a DST gap), so the caller keeps the
 * last good chart on the plate and glides the rings anyway. */
export type Outcome =
  | {
      ok: true;
      chart: ChartData;
      warnings: string[];
      /** The instant this chart was computed for — not necessarily the draft's
       * current value, which may have moved on. */
      minutes: number;
      /** True for the very first chart of the session. It appears in place; the
       * wheel's own entrance choreography is its arrival. Everything after
       * glides. */
      first: boolean;
      source: MoveSource;
    }
  | { ok: false; reason: string; minutes: number; source: MoveSource };

/** What the pump needs from the world around it. */
export type Ports = {
  /** The request for the draft as it stands *now*, read fresh before every
   * round — the draft moves while a request is in flight, and the point of the
   * pump is that the next round asks for where it ended up, not where it was. */
  next: () => { input: PreviewInput; minutes: number } | null;
  /** The transport. Production passes `api.preview`; tests pass a fake. */
  send: (input: PreviewInput) => Promise<{ chart: ChartData; warnings: string[] }>;
  /** Called once per round, with what it produced. */
  settle: (outcome: Outcome) => void;
  /** Whether a request is in flight, for the "computing" affordances. */
  pending: (pending: boolean) => void;
};

export type Pump = {
  /** Ask for a preview of the current draft. Safe to call at drag rates. */
  request: (source: MoveSource) => void;
};

/** Trailing-edge coalescing: at most one request in flight, and while one is,
 * further asks collapse into exactly one follow-up round that fires when it
 * lands. The last ask's `source` wins, because that is the gesture whose easing
 * the arrival should use.
 *
 * A stale response cannot overwrite a newer one, and no counter is needed to
 * ensure it: rounds run in sequence, so at any moment there is exactly one
 * outstanding request. (The version this replaced carried a `seq` guard for
 * this, which the coalescing had already made unreachable.) */
export function createPump(ports: Ports): Pump {
  let running = false;
  let queued: MoveSource | null = null;
  let first = true;

  async function run(source: MoveSource) {
    running = true;
    ports.pending(true);
    let src: MoveSource | null = source;
    while (src !== null) {
      const asked = ports.next();
      if (asked !== null) {
        const { input, minutes } = asked;
        try {
          const { chart, warnings } = await ports.send(input);
          ports.settle({ ok: true, chart, warnings, minutes, first, source: src });
          first = false;
        } catch (e) {
          ports.settle({ ok: false, reason: reason(e), minutes, source: src });
        }
      }
      src = queued;
      queued = null;
    }
    ports.pending(false);
    running = false;
  }

  return {
    request(source: MoveSource) {
      // Nothing to ask for yet — don't spin up a round that would no-op and
      // flicker `pending`.
      if (ports.next() === null) return;
      if (running) {
        queued = source;
        return;
      }
      void run(source);
    },
  };
}
