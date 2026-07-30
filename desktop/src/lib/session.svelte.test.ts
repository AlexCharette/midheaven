import { describe, expect, it, vi } from "vitest";
import type { ChartData } from "./types";

// `session.svelte.ts` holds one module-level `$state`, so it is a singleton:
// each test re-imports it fresh, together with `state.svelte.ts` so both halves
// share the same module instance and `toasts` reflects this session's refusals.
async function fresh() {
  vi.resetModules();
  const session = await import("./session.svelte");
  const state = await import("./state.svelte");
  return { session, toasts: state.toasts };
}

const chart = (name: string) => ({ meta: { name }, excerpts: [] }) as unknown as ChartData;

describe("the reactive session starts empty", () => {
  it("has no reading and is not recording", async () => {
    const { session } = await fresh();
    expect(session.chart()).toBeNull();
    expect(session.hasReading()).toBe(false);
    expect(session.isRecording()).toBe(false);
  });
});

describe("transitions apply to the store", () => {
  it("opening a reading is visible through the reads", async () => {
    const { session } = await fresh();
    expect(session.openReading(chart("A"))).toBe(true);
    expect(session.chart()?.meta.name).toBe("A");
    expect(session.hasReading()).toBe(true);
  });

  it("a take begins and ends, keeping the reading throughout", async () => {
    const { session } = await fresh();
    session.openReading(chart("A"));
    expect(session.takeBegan()).toBe(true);
    expect(session.isRecording()).toBe(true);
    expect(session.chart()?.meta.name).toBe("A");

    expect(session.takeEnded(chart("routed"))).toBe(true);
    expect(session.isRecording()).toBe(false);
    expect(session.chart()?.meta.name).toBe("routed");
  });

  it("a failed transcription ends the take and keeps the chart", async () => {
    const { session } = await fresh();
    session.openReading(chart("A"));
    session.takeBegan();
    expect(session.takeEnded(null)).toBe(true);
    expect(session.isRecording()).toBe(false);
    expect(session.chart()?.meta.name).toBe("A");
  });

  it("curation replaces the chart without ending a take", async () => {
    const { session } = await fresh();
    session.openReading(chart("A"));
    session.takeBegan();
    expect(session.updateChart(chart("curated"))).toBe(true);
    expect(session.isRecording()).toBe(true);
    expect(session.chart()?.meta.name).toBe("curated");
  });

  it("leaving clears the reading", async () => {
    const { session } = await fresh();
    session.openReading(chart("A"));
    expect(session.leaveReading()).toBe(true);
    expect(session.chart()).toBeNull();
    expect(session.hasReading()).toBe(false);
  });
});

describe("refusals do not mutate, and are never silent", () => {
  // The guards drive each control's `disabled`, so a refusal reaching here means
  // a control was left ungated — it must be visible rather than a no-op.
  it("leaving mid-take is refused, toasted, and leaves the session alone", async () => {
    const { session, toasts } = await fresh();
    session.openReading(chart("A"));
    session.takeBegan();
    const before = toasts.length;

    expect(session.leaveReading()).toBe(false);
    expect(session.isRecording()).toBe(true);
    expect(session.chart()?.meta.name).toBe("A");

    expect(toasts.length).toBe(before + 1);
    const last = toasts[toasts.length - 1];
    expect(last.kind).toBe("error");
    expect(last.message).toBe("stop transcribing first");
  });

  it("opening another reading mid-take is refused and keeps the first", async () => {
    const { session, toasts } = await fresh();
    session.openReading(chart("A"));
    session.takeBegan();
    const before = toasts.length;

    expect(session.openReading(chart("B"))).toBe(false);
    expect(session.chart()?.meta.name).toBe("A");
    expect(toasts.length).toBe(before + 1);
  });

  it("curating with nothing open is refused", async () => {
    const { session } = await fresh();
    expect(session.updateChart(chart("X"))).toBe(false);
    expect(session.chart()).toBeNull();
  });
});

describe("guards track the live phase", () => {
  // These are what the markup binds to `disabled` and `title`, so they have to
  // move with the phase rather than being read once.
  it("refusal reasons appear and clear as recording starts and stops", async () => {
    const { session } = await fresh();
    session.openReading(chart("A"));
    expect(session.whyCannotLeave()).toBeNull();
    expect(session.whyCannotOpenAnother()).toBeNull();
    expect(session.whyCannotRecalculate()).toBeNull();
    expect(session.whyCannotRecord()).toBeNull();

    session.takeBegan();
    expect(session.whyCannotLeave()).toBe("stop transcribing first");
    expect(session.whyCannotOpenAnother()).toBe("stop transcribing first");
    expect(session.whyCannotRecalculate()).toBe("stop transcribing first");
    expect(session.whyCannotRecord()).toBe("already transcribing");

    session.takeEnded(chart("routed"));
    expect(session.whyCannotLeave()).toBeNull();
    expect(session.whyCannotRecalculate()).toBeNull();
    expect(session.whyCannotRecord()).toBeNull();
  });

  it("recalculating and recording are unavailable with nothing open", async () => {
    const { session } = await fresh();
    expect(session.whyCannotRecalculate()).toBe("no reading is open");
    expect(session.whyCannotRecord()).toBe("no reading is open");
    // ...but leaving an empty session is not an error, so nothing to refuse.
    expect(session.whyCannotLeave()).toBeNull();
  });
});
