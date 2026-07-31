import { describe, expect, it } from "vitest";
import { norm360, positionOf, sweepOf } from "./derive";

// `derive.ts` is a mirror of `src/derive.rs`. These cases are the Rust module's
// own cases, so the two cannot drift silently: if a value below changes, the
// corresponding Rust test changed too.

describe("norm360", () => {
  it("normalizes into [0, 360), including the values a hand-edited chart carries", () => {
    expect(norm360(40)).toBe(40);
    expect(norm360(400)).toBe(40);
    expect(norm360(-30)).toBe(330);
    expect(norm360(360)).toBe(0);
    expect(norm360(-720)).toBe(0);
    for (const raw of [-1000, -0.5, 0, 359.999, 360, 1e4]) {
      const d = norm360(raw);
      expect(d, `${raw}`).toBeGreaterThanOrEqual(0);
      expect(d, `${raw}`).toBeLessThan(360);
    }
  });
});

describe("positionOf", () => {
  it("splits a longitude into sign, degree and minute", () => {
    expect(positionOf(0)).toEqual({ sign: 0, deg: 0, min: 0 });
    // 17°30' Cancer — Cancer is the 4th sign, index 3, spanning [90, 120)
    expect(positionOf(107.5)).toEqual({ sign: 3, deg: 17, min: 30 });
    // last whole degree of Pisces
    expect(positionOf(359)).toEqual({ sign: 11, deg: 29, min: 0 });
  });

  it("carries rounded minutes into the next sign", () => {
    // 29.9999° into Cancer: minutes round to 60, the degree would become 30
    // (which does not exist), so the carry advances the sign.
    expect(positionOf(119.9999)).toEqual({ sign: 4, deg: 0, min: 0 });
    // The same carry at the end of the zodiac wraps back to Aries.
    expect(positionOf(359.9999)).toEqual({ sign: 0, deg: 0, min: 0 });
    // A carry that only advances the degree stays inside its sign.
    expect(positionOf(100.99999)).toEqual({ sign: 3, deg: 11, min: 0 });
  });

  it("never leaves its ranges anywhere on the circle", () => {
    for (let i = 0; i < 36_000; i++) {
      const p = positionOf(i / 100);
      expect(p.sign, `sign at ${i}`).toBeLessThan(12);
      expect(p.deg, `deg at ${i}`).toBeLessThan(30);
      expect(p.min, `min at ${i}`).toBeLessThan(60);
    }
  });

  it("agrees with the arithmetic the wheel used to inline", () => {
    // The copies this module replaces, reproduced: floor the degree, round the
    // minute, and stop — no carry past 30°.
    const legacy = (lon: number) => {
      const within = norm360(lon) % 30;
      let d = Math.floor(within);
      let m = Math.round((within - d) * 60);
      if (m >= 60) {
        d += 1;
        m = 0;
      }
      return { sign: Math.floor(norm360(lon) / 30) % 12, deg: d, min: m };
    };
    // A 0.01° grid never reaches the carry, so every sample is an ordinary
    // position and must be unchanged — that is what makes the swap safe.
    for (let i = 0; i < 36_000; i++) {
      const lon = i / 100;
      const l = legacy(lon);
      expect(l.deg, `grid hit the carry at ${lon}`).toBeLessThan(30);
      expect(positionOf(lon), `drift at ${lon}`).toEqual(l);
    }
    // And the one place it deliberately differs: inside a sign's final
    // arcminute the old copies printed a 30th degree no sign has.
    for (const lon of [29.9999, 119.9999, 119.9958, 359.9999, 209.99306, 89.995]) {
      expect(legacy(lon).deg, `legacy should carry to 30 at ${lon}`).toBe(30);
      const p = positionOf(lon);
      expect(p.deg).toBe(0);
      expect(p.sign).toBe((legacy(lon).sign + 1) % 12);
    }
  });
});

describe("sweepOf", () => {
  it("wraps, and treats coincident cusps as a full sign", () => {
    expect(sweepOf(0, 30)).toBe(30);
    // the twelfth house closing the circle
    expect(sweepOf(330, 0)).toBe(30);
    // a collapsed quadrant cusp spans its sign rather than vanishing
    expect(sweepOf(212.5, 212.5)).toBe(30);
    // an intermediate quadrant width passes through untouched
    expect(sweepOf(10, 48)).toBe(38);
  });

  it("tiles the circle exactly once over a full cusp ring", () => {
    const rings = [
      Array.from({ length: 12 }, (_, i) => i * 30),
      [12, 40, 71, 102, 140, 173, 192, 220, 251, 282, 320, 353],
    ];
    for (const cusps of rings) {
      const total = cusps.reduce((sum, c, i) => sum + sweepOf(c, cusps[(i + 1) % 12]), 0);
      expect(total).toBeCloseTo(360, 9);
    }
  });
});
