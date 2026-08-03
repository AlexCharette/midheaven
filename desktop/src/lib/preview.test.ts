import { describe, expect, it } from "vitest";
import { toMinutes } from "./civil";
import { createPump, requestFor } from "./preview";
import type { Draft, Outcome, Ports } from "./preview";
import type { ChartData, PlaceDto, PreviewInput } from "./types";

const berlin = { id: 2950159, label: "Berlin, Germany" } as PlaceDto;

const draftAt = (minutes: number, over: Partial<Draft> = {}): Draft => ({
  minutes,
  place: berlin,
  houseSystem: "whole-sign",
  zodiac: "tropical",
  ayanamsa: "lahiri",
  lang: "en",
  ...over,
});

const chartFor = (label: string) => ({ meta: { name: label } }) as unknown as ChartData;

/** A pump wired to a transport we drive by hand, recording every outcome. */
function harness(opts: { draft?: () => Draft } = {}) {
  const outcomes: Outcome[] = [];
  const pendingLog: boolean[] = [];
  const sent: PreviewInput[] = [];
  /** Each call parks until the test resolves or rejects it. */
  const inbox: {
    resolve: (v: { chart: ChartData; warnings: string[] }) => void;
    reject: (e: unknown) => void;
  }[] = [];

  let current = draftAt(1000);
  const ports: Ports = {
    next: () => {
      const d = opts.draft ? opts.draft() : current;
      const input = requestFor(d);
      return input === null ? null : { input, minutes: d.minutes };
    },
    send: (input) => {
      sent.push(input);
      return new Promise((resolve, reject) => inbox.push({ resolve, reject }));
    },
    settle: (o) => outcomes.push(o),
    pending: (p) => pendingLog.push(p),
  };

  return {
    pump: createPump(ports),
    outcomes,
    pendingLog,
    sent,
    inbox,
    setDraft: (min: number, over: Partial<Draft> = {}) => (current = draftAt(min, over)),
    /** Answer the oldest outstanding request. */
    async land(label: string, warnings: string[] = []) {
      const next = inbox.shift();
      if (!next) throw new Error("nothing in flight");
      next.resolve({ chart: chartFor(label), warnings });
      await Promise.resolve();
      await Promise.resolve();
    },
    async fail(reason: string) {
      const next = inbox.shift();
      if (!next) throw new Error("nothing in flight");
      next.reject(reason);
      await Promise.resolve();
      await Promise.resolve();
    },
  };
}

describe("what a preview asks for", () => {
  it("is nothing until a place and a moment are both there", () => {
    expect(requestFor(draftAt(0))).toBeNull();
    expect(requestFor(draftAt(1000, { place: null }))).toBeNull();
    expect(requestFor(draftAt(1000))).not.toBeNull();
  });

  it("sends the ayanamsa only for a sidereal zodiac", () => {
    expect(requestFor(draftAt(1000))?.ayanamsa).toBeNull();
    expect(requestFor(draftAt(1000, { zodiac: "sidereal" }))?.ayanamsa).toBe("lahiri");
  });

  it("splits the civil instant into the date and time the backend expects", () => {
    const r = requestFor(draftAt(toMinutes("1990-07-13", "14:30")!));
    expect(r?.date).toBe("1990-07-13");
    expect(r?.time).toBe("14:30");
    expect(r?.place_id).toBe(berlin.id);
  });
});

describe("the pump keeps one request in flight", () => {
  it("sends immediately when idle", async () => {
    const h = harness();
    h.pump.request("field");
    expect(h.sent).toHaveLength(1);
    expect(h.pendingLog).toEqual([true]);
    await h.land("a");
    expect(h.outcomes).toHaveLength(1);
    expect(h.pendingLog).toEqual([true, false]);
  });

  it("collapses a burst of asks into exactly one follow-up", async () => {
    const h = harness();
    h.pump.request("drag");
    expect(h.sent).toHaveLength(1);

    // Ten more drag frames arrive while the first request is out.
    for (let i = 0; i < 10; i++) {
      h.setDraft(2000 + i);
      h.pump.request("drag");
    }
    expect(h.sent, "no request may be sent while one is in flight").toHaveLength(1);

    await h.land("first");
    // Exactly one follow-up, for where the draft ENDED UP — not eleven.
    expect(h.sent).toHaveLength(2);
    expect(h.sent[1].time).not.toBe(h.sent[0].time);
    await h.land("second");
    expect(h.sent).toHaveLength(2);
    expect(h.outcomes).toHaveLength(2);
  });

  it("stays pending across the whole burst, then settles once", async () => {
    const h = harness();
    h.pump.request("drag");
    h.setDraft(2000);
    h.pump.request("drag");
    expect(h.pendingLog).toEqual([true]);
    await h.land("first");
    expect(h.pendingLog, "must not flicker between rounds").toEqual([true]);
    await h.land("second");
    expect(h.pendingLog).toEqual([true, false]);
  });

  it("asks for where the draft is now, not where it was when the ask came in", async () => {
    const h = harness();
    h.pump.request("drag");
    h.setDraft(5000);
    h.pump.request("drag");
    h.setDraft(9999); // moved again before the first landed
    await h.land("first");
    const asked = requestFor(draftAt(9999));
    expect(h.sent[1].date).toBe(asked!.date);
    expect(h.sent[1].time).toBe(asked!.time);
  });

  it("takes the easing of the LAST gesture in the burst", async () => {
    const h = harness();
    h.pump.request("drag");
    h.setDraft(2000);
    h.pump.request("drag");
    h.setDraft(3000);
    h.pump.request("keyboard");
    await h.land("first");
    await h.land("second");
    expect(h.outcomes.map((o) => o.source)).toEqual(["drag", "keyboard"]);
  });

  it("never sends when there is nothing to ask for, and never claims to be busy", () => {
    const h = harness({ draft: () => draftAt(0) });
    h.pump.request("now");
    expect(h.sent).toHaveLength(0);
    expect(h.pendingLog).toEqual([]);
  });

  it("goes quiet again after a burst, ready for the next one", async () => {
    const h = harness();
    h.pump.request("drag");
    h.setDraft(2000);
    h.pump.request("drag");
    await h.land("first");
    await h.land("second");
    h.setDraft(3000);
    h.pump.request("field");
    expect(h.sent).toHaveLength(3);
  });
});

describe("the first arrival, and the ones after", () => {
  it("marks only the very first chart as first — it appears in place, the rest glide", async () => {
    const h = harness();
    h.pump.request("now");
    await h.land("a");
    h.setDraft(2000);
    h.pump.request("drag");
    await h.land("b");
    expect(h.outcomes.map((o) => o.ok && o.first)).toEqual([true, false]);
  });

  it("a failed first attempt does not spend the first-arrival snap", async () => {
    const h = harness();
    h.pump.request("now");
    await h.fail("1990-03-25 02:30 does not exist in Europe/Berlin (DST gap)");
    expect(h.outcomes[0].ok).toBe(false);

    h.setDraft(2000);
    h.pump.request("field");
    await h.land("a");
    // Still the first chart the plate has ever shown, so it must not glide in
    // from the placeholder pose.
    expect(h.outcomes[1].ok && h.outcomes[1].first).toBe(true);
  });
});

describe("an uncomputable moment", () => {
  it("reports the reason and the instant, and keeps pumping", async () => {
    const h = harness();
    h.pump.request("field");
    await h.fail("does not exist in Europe/Berlin (DST gap)");

    const o = h.outcomes[0];
    expect(o.ok).toBe(false);
    if (!o.ok) {
      expect(o.reason).toContain("DST gap");
      expect(o.minutes).toBe(1000);
      expect(o.source).toBe("field");
    }
    expect(h.pendingLog).toEqual([true, false]);

    // A failure is not a dead end: the next moment is asked for as usual.
    h.setDraft(4000);
    h.pump.request("drag");
    expect(h.sent).toHaveLength(2);
    await h.land("recovered");
    expect(h.outcomes[1].ok).toBe(true);
  });

  it("a failure mid-burst does not swallow the follow-up round", async () => {
    const h = harness();
    h.pump.request("drag");
    h.setDraft(2000);
    h.pump.request("drag");
    await h.fail("DST gap");
    expect(h.sent).toHaveLength(2);
    await h.land("second");
    expect(h.outcomes.map((o) => o.ok)).toEqual([false, true]);
  });
});
