import { describe, expect, it } from "vitest";
import { angleLerp, lerpPose, type Pose } from "./pose";

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
