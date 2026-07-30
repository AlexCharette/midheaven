import { describe, expect, it } from "vitest";
import type { ChartData } from "./types";
import {
  IDLE,
  chartOf,
  chartReplaced,
  closed,
  isRecording,
  opened,
  recordingStarted,
  recordingStopped,
  whyCannotLeave,
  whyCannotOpenAnother,
  whyCannotRecalculate,
  whyCannotRecord,
  type Session,
} from "./session";

// The machine holds a chart opaquely and never inspects it, so a labelled stub
// is enough to tell two charts apart.
const chart = (name: string) => ({ meta: { name } }) as unknown as ChartData;

/** Advance through a transition, failing the test if it refused. */
function must(t: ReturnType<typeof opened>): Session {
  if (!t.ok) throw new Error(`unexpected refusal: ${t.reason}`);
  return t.session;
}

const openWith = (name = "A") => must(opened(IDLE, chart(name)));
const recordingWith = (name = "A") => must(recordingStarted(openWith(name)));

describe("reading the session", () => {
  it("reports the open chart, or null when idle", () => {
    expect(chartOf(IDLE)).toBeNull();
    expect(chartOf(openWith("A"))?.meta.name).toBe("A");
    expect(chartOf(recordingWith("A"))?.meta.name).toBe("A");
  });

  it("a recording session always has a chart", () => {
    const s = recordingWith();
    expect(isRecording(s)).toBe(true);
    expect(chartOf(s)).not.toBeNull();
  });
});

describe("leaving the reading — the stranded recorder", () => {
  // The defect this machine exists to remove. The old model was two independent
  // client facts, and `back()` wrote only one of them:
  //
  //   function back() { app.chart = null; /* `recording` untouched */ }
  //
  // The stop control lived inside `{#if app.chart}`, so it unmounted while the
  // backend kept capturing — and the take then landed on the next chart opened.
  it("refuses to leave while recording", () => {
    const t = closed(recordingWith());
    expect(t.ok).toBe(false);
    if (!t.ok) expect(t.reason).toBe("stop transcribing first");
  });

  it("allows leaving once recording has stopped", () => {
    const stopped = must(recordingStopped(recordingWith(), chart("routed")));
    const t = closed(stopped);
    expect(t.ok).toBe(true);
    if (t.ok) expect(t.session).toEqual(IDLE);
  });

  it("leaving an idle session is a no-op, not an error", () => {
    const t = closed(IDLE);
    expect(t.ok).toBe(true);
    if (t.ok) expect(t.session).toEqual(IDLE);
  });

  // The property that actually matters, and it is about reachability rather
  // than shape. "Recording with no chart" is unrepresentable in the union, so
  // asserting its absence would be tautological — it could never fail. What a
  // test can check is that recording is never left implicitly: the only way out
  // of `recording` is `recordingStopped`, which is what tells the backend's
  // recorder and the client's phase to end together.
  it("recording can only be left by stopping it", () => {
    const s = recordingWith();
    const others = {
      closed: closed(s),
      opened: opened(s, chart("B")),
      recordingStarted: recordingStarted(s),
      chartReplaced: chartReplaced(s, chart("D")),
    };
    for (const [name, t] of Object.entries(others)) {
      if (t.ok) {
        expect(t.session.phase, `${name} must not leave recording`).toBe("recording");
      }
    }
    // ...and stopping does leave it.
    expect(must(recordingStopped(s, chart("C"))).phase).toBe("open");
  });
});

describe("opening another reading", () => {
  it("refuses while recording — the library path", () => {
    const t = opened(recordingWith("A"), chart("B"));
    expect(t.ok).toBe(false);
    if (!t.ok) expect(t.reason).toBe("stop transcribing first");
  });

  it("replaces the chart when merely open", () => {
    const t = opened(openWith("A"), chart("B"));
    expect(t.ok).toBe(true);
    if (t.ok) expect(chartOf(t.session)?.meta.name).toBe("B");
  });
});

describe("recalculating", () => {
  it("refuses while recording — the reproject-in-the-mutex-gap path", () => {
    expect(whyCannotRecalculate(recordingWith())).toBe("stop transcribing first");
  });

  it("is available on an open reading and unavailable when idle", () => {
    expect(whyCannotRecalculate(openWith())).toBeNull();
    expect(whyCannotRecalculate(IDLE)).toBe("no reading is open");
  });
});

describe("recording transitions", () => {
  it("cannot start without a reading open", () => {
    const t = recordingStarted(IDLE);
    expect(t.ok).toBe(false);
    if (!t.ok) expect(t.reason).toBe("no reading is open");
  });

  it("cannot start twice", () => {
    const t = recordingStarted(recordingWith());
    expect(t.ok).toBe(false);
    if (!t.ok) expect(t.reason).toBe("already transcribing");
  });

  it("stopping files the routed chart", () => {
    const s = must(recordingStopped(recordingWith("A"), chart("routed")));
    expect(s.phase).toBe("open");
    expect(chartOf(s)?.meta.name).toBe("routed");
  });

  // The recorder has stopped whether or not transcription succeeded, so the
  // phase must leave `recording` either way — otherwise a failed take would
  // leave the UI showing a stop button for a recorder that no longer exists.
  it("stopping after a failed transcription keeps the previous chart", () => {
    const s = must(recordingStopped(recordingWith("A"), null));
    expect(s.phase).toBe("open");
    expect(chartOf(s)?.meta.name).toBe("A");
  });

  it("cannot stop what is not recording", () => {
    for (const s of [IDLE, openWith()]) {
      const t = recordingStopped(s, chart("X"));
      expect(t.ok).toBe(false);
      if (!t.ok) expect(t.reason).toBe("not transcribing");
    }
  });
});

describe("curation during a take", () => {
  // Curation is not one of the refused paths: merging or amending a passage
  // mid-take returns an updated chart and must not end the recording.
  it("keeps the phase while replacing the chart", () => {
    const s = must(chartReplaced(recordingWith("A"), chart("curated")));
    expect(s.phase).toBe("recording");
    expect(chartOf(s)?.meta.name).toBe("curated");
  });

  it("works on an open reading too", () => {
    const s = must(chartReplaced(openWith("A"), chart("curated")));
    expect(s.phase).toBe("open");
    expect(chartOf(s)?.meta.name).toBe("curated");
  });

  it("refuses when nothing is open", () => {
    const t = chartReplaced(IDLE, chart("X"));
    expect(t.ok).toBe(false);
    if (!t.ok) expect(t.reason).toBe("no reading is open");
  });
});

describe("guards agree with the transitions they gate", () => {
  // A guard that says "allowed" while its transition refuses (or the reverse)
  // would put the UI out of step with what the machine will actually do — a
  // disabled button that would have worked, or an enabled one that fails.
  it("every guard matches its transition on every phase", () => {
    for (const s of [IDLE, openWith(), recordingWith()]) {
      expect(whyCannotLeave(s) === null).toBe(closed(s).ok);
      expect(whyCannotOpenAnother(s) === null).toBe(opened(s, chart("B")).ok);
      expect(whyCannotRecord(s) === null).toBe(recordingStarted(s).ok);
    }
  });

  it("refusal reasons are non-empty", () => {
    for (const s of [IDLE, openWith(), recordingWith()]) {
      for (const t of [closed(s), opened(s, chart("B")), recordingStarted(s)]) {
        if (!t.ok) expect(t.reason.length).toBeGreaterThan(0);
      }
    }
  });
});
