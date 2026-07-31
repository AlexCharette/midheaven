import { describe, expect, it } from "vitest";
import { angleLerp, lerpPose, poseOf, project, type Pose } from "./pose";
import { ringAngles } from "./civil";
import type { ChartData } from "./types";

describe("angleLerp", () => {
  it("takes the shortest arc across 0°", () => {
    expect(angleLerp(350, 10, 0.5)).toBe(360); // +20° forward, not −340°
    expect(angleLerp(10, 350, 0.5)).toBe(0); // −20° backward, not +340°
    expect(angleLerp(10, 350, 1)).toBe(-10); // lands on 350 ≡ −10
  });

  it("caps any jump at 180°", () => {
    // Directly opposite points move by exactly half a turn, never more.
    expect(Math.abs(angleLerp(0, 180, 1) - 0)).toBeLessThanOrEqual(180);
    expect(Math.abs(angleLerp(90, 271, 0.5) - 90)).toBeLessThan(90.5);
  });

  it("ends equal to the target only modulo 360", () => {
    const end = angleLerp(350, 10, 1);
    expect(end).toBe(370); // === 10 is false — the documented trap
    expect(((end % 360) + 360) % 360).toBeCloseTo(10, 9);
  });

  it("handles negative accumulations (a mid-flight retarget start)", () => {
    expect(((angleLerp(-10, 10, 1) % 360) + 360) % 360).toBeCloseTo(10, 9);
  });
});

describe("lerpPose", () => {
  const a: Pose = {
    asc: 350,
    mc: 260,
    cusps: [350, 20, 50, 80, 110, 140, 170, 200, 230, 260, 290, 320],
    lons: { "planet:sun": 10, "planet:moon": 200 },
    timeAngle: 359,
    dateAngle: 100,
  };
  const b: Pose = {
    asc: 10,
    mc: 280,
    cusps: [10, 40, 70, 100, 130, 160, 190, 220, 250, 280, 310, 340],
    lons: { "planet:sun": 30, "planet:moon": 190 },
    timeAngle: 1,
    dateAngle: 101,
  };

  it("returns the exact target at t=1", () => {
    expect(lerpPose(a, b)(1)).toEqual(b);
  });

  it("interpolates every quantity on the same clock, wrap-safely", () => {
    const mid = lerpPose(a, b)(0.5);
    expect(mid.asc).toBeCloseTo(360, 1); // 350→10 through 0°, not backward
    expect(mid.timeAngle).toBeCloseTo(360, 1); // 359→1 the short way
    expect(mid.lons["planet:sun"]).toBeCloseTo(20, 1);
    expect(mid.lons["planet:moon"]).toBeCloseTo(195, 1);
    expect(mid.cusps[0]).toBeCloseTo(360, 1);
  });

  it("lands new bodies in place instead of sweeping from 0°", () => {
    const grown: Pose = { ...b, lons: { ...b.lons, "planet:chiron": 123 } };
    expect(lerpPose(a, grown)(0.5).lons["planet:chiron"]).toBe(123);
  });

  it("quantizes so settled frames repeat exactly", () => {
    const f = lerpPose(a, b);
    expect(f(0.500001)).toEqual(f(0.500002));
  });
});

/** A chart just real enough to pose and project. */
function chart(): ChartData {
  return {
    meta: { name: "T", locale: "en" },
    axes: { asc: 10, mc: 280 },
    houseCusps: [10, 40, 70, 100, 130, 160, 190, 220, 250, 280, 310, 340],
    houseSweeps: new Array(12).fill(30),
    signs: Array.from({ length: 12 }, (_, i) => ({
      id: `sign:s${i}`,
      glyph: "x",
      name: `S${i}`,
      element: "fire",
    })),
    houses: Array.from({ length: 12 }, (_, i) => ({
      id: `house:${i + 1}`,
      label: "I",
      name: `H${i + 1}`,
    })),
    planets: [
      { id: "planet:sun", glyph: "☉", name: "Sun", lon: 107.5, house: 4, sign: 3, deg: 17, min: 30 },
      { id: "planet:moon", glyph: "☽", name: "Moon", lon: 350, house: 12, sign: 11, deg: 20, min: 0 },
    ],
    aspects: [],
    excerpts: [],
  } as unknown as ChartData;
}

describe("poseOf", () => {
  it("lifts every angular quantity out of the chart", () => {
    const p = poseOf(chart(), 1000);
    expect(p.asc).toBe(10);
    expect(p.mc).toBe(280);
    expect(p.cusps).toEqual([10, 40, 70, 100, 130, 160, 190, 220, 250, 280, 310, 340]);
    expect(p.lons).toEqual({ "planet:sun": 107.5, "planet:moon": 350 });
  });

  it("copies the cusps rather than aliasing the chart's array", () => {
    const c = chart();
    const p = poseOf(c, 0);
    p.cusps[0] = 999;
    expect(c.houseCusps[0]).toBe(10);
  });

  it("carries the instrument-ring angles for the instant", () => {
    const p = poseOf(chart(), 1000);
    expect(p).toMatchObject(ringAngles(1000));
  });
});

describe("project", () => {
  it("keeps the target's identity and takes the pose's angles", () => {
    const target = chart();
    const posed = { ...poseOf(target, 0), asc: 40, lons: { "planet:sun": 200 } };
    const out = project(target, posed);

    expect(out.meta).toBe(target.meta);
    expect(out.axes.asc).toBe(40);
    // A body the pose does not mention stays where the target had it.
    expect(out.planets[1].lon).toBe(350);
    expect(out.planets[0].lon).toBe(200);
  });

  it("re-derives each body's position from its tweened longitude", () => {
    const target = chart();
    // Mid-glide the Sun sits at 200° — 20° into the 7th sign, not where the
    // target's own `sign`/`deg` say.
    const out = project(target, { ...poseOf(target, 0), lons: { "planet:sun": 200 } });
    expect(out.planets[0]).toMatchObject({ sign: 6, deg: 20, min: 0 });
    // The target itself is untouched.
    expect(target.planets[0]).toMatchObject({ sign: 3, deg: 17, min: 30 });
  });

  it("re-derives the house sweeps from the tweened cusps", () => {
    const target = chart();
    // Cusps mid-glide from whole-sign toward an uneven ring.
    const cusps = [0, 20, 50, 100, 130, 160, 190, 220, 250, 280, 310, 340];
    const out = project(target, { ...poseOf(target, 0), cusps });
    expect(out.houseSweeps[0]).toBe(20);
    expect(out.houseSweeps[1]).toBe(30);
    // the twelfth closes the circle back to 0°
    expect(out.houseSweeps[11]).toBe(20);
    expect(out.houseSweeps.reduce((a, b) => a + b, 0)).toBeCloseTo(360, 9);
  });

  it("normalizes the pose's out-of-range angles, which lerping produces", () => {
    // `angleLerp` deliberately leaves [0,360) — a pose value of −10° is 350°.
    const target = chart();
    const out = project(target, {
      ...poseOf(target, 0),
      asc: -10,
      cusps: new Array(12).fill(0).map((_, i) => 370 + i * 30),
      lons: { "planet:sun": -20 },
    });
    expect(out.axes.asc).toBe(350);
    expect(out.houseCusps[0]).toBe(10);
    expect(out.planets[0].lon).toBe(340);
    expect(out.planets[0].sign).toBe(11);
  });
});
