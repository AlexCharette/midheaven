import { describe, expect, it } from "vitest";
import { PLATE } from "./generated/plate";
import { angularGap, axisLongitudes, crowdedRadii, densityLength, focusLongitude } from "./orrery";
import type { ChartData } from "./types";

const C = PLATE.crowding;
const PLANET = PLATE.radii.planet;

/** A chart just real enough for the placement rules. */
function chart(over: Partial<ChartData> = {}): ChartData {
  return {
    axes: { asc: 10, mc: 280 },
    houseCusps: [0, 20, 50, 100, 130, 160, 190, 220, 250, 280, 310, 340],
    houseSweeps: [20, 30, 50, 30, 30, 30, 30, 30, 30, 30, 30, 20],
    signs: Array.from({ length: 12 }, (_, i) => ({ id: `sign:s${i}`, glyph: "x", name: "S", element: "fire" })),
    houses: Array.from({ length: 12 }, (_, i) => ({ id: `house:${i + 1}`, label: "I", name: "H" })),
    planets: [
      { id: "planet:sun", glyph: "x", name: "Sun", lon: 10, house: 1, sign: 0, deg: 10, min: 0 },
      { id: "planet:moon", glyph: "x", name: "Moon", lon: 200, house: 7, sign: 6, deg: 20, min: 0 },
    ],
    aspects: [
      { id: "aspect:sun-moon", glyph: "x", name: "Trine", a: "planet:sun", b: "planet:moon", nature: "harmonious", orb: 0 },
    ],
    excerpts: [],
    ...over,
  } as unknown as ChartData;
}

describe("angular gap", () => {
  it("is the shorter way round", () => {
    expect(angularGap(10, 40)).toBe(30);
    expect(angularGap(40, 10)).toBe(30);
    // The case a plain subtraction gets wrong.
    expect(angularGap(359, 2)).toBe(3);
    expect(angularGap(2, 359)).toBe(3);
  });

  it("never exceeds half a turn", () => {
    for (let a = 0; a < 360; a += 7) {
      for (let b = 0; b < 360; b += 11) {
        const g = angularGap(a, b);
        expect(g).toBeGreaterThanOrEqual(0);
        expect(g).toBeLessThanOrEqual(180);
      }
    }
  });
});

describe("de-crowding", () => {
  it("leaves bodies that are far apart on the planet ring", () => {
    expect(crowdedRadii([0, 90, 180, 270])).toEqual([PLANET, PLANET, PLANET, PLANET]);
  });

  it("steps a crowded body in from the one before it", () => {
    const [first, second] = crowdedRadii([100, 101]);
    expect(first).toBe(PLANET);
    expect(second).toBeLessThan(PLANET);
  });

  /// The orrery's stated departure from the plate: the specification steps at
  /// its threshold, this ramps in over the three degrees above it, so a body
  /// closing during a live scrub slides rather than pops.
  it("ramps in over the three degrees above the threshold", () => {
    const at = (gap: number) => crowdedRadii([100, 100 + gap])[1];
    expect(at(C.thresholdDeg + 3.5)).toBe(PLANET);
    const justInside = at(C.thresholdDeg + 2);
    const atThreshold = at(C.thresholdDeg);
    const wellInside = at(1);
    expect(justInside).toBeLessThan(PLANET);
    expect(atThreshold).toBeLessThan(justInside);
    expect(wellInside).toBeLessThanOrEqual(atThreshold);
    // A static chart renders as the spec does: at or below the threshold the
    // step is the full one.
    expect(wellInside).toBe(PLANET - C.step);
  });

  it("cascades — a stellium stacks one behind another", () => {
    const radii = crowdedRadii([100, 100.2, 100.4, 100.6]);
    expect(radii[0]).toBe(PLANET);
    for (let i = 1; i < radii.length; i++) expect(radii[i]).toBeLessThan(radii[i - 1]);
  });

  it("never stacks past the floor, however many bodies pile up", () => {
    const radii = crowdedRadii(Array.from({ length: 40 }, (_, i) => 100 + i * 0.05));
    expect(Math.min(...radii)).toBeGreaterThanOrEqual(C.floor);
    expect(Math.min(...radii)).toBeGreaterThan(PLATE.radii.hub);
  });

  it("counts the wrap — bodies either side of 0° are crowded", () => {
    expect(crowdedRadii([359, 1])[1]).toBeLessThan(PLANET);
  });

  it("gives one radius per body, in the order handed to it", () => {
    expect(crowdedRadii([])).toEqual([]);
    expect(crowdedRadii([42])).toEqual([PLANET]);
    expect(crowdedRadii([0, 1, 2])).toHaveLength(3);
  });
});

describe("the density bar", () => {
  it("is nothing for a sign the reading never touched", () => {
    expect(densityLength(0, 5, 100)).toBe(0);
  });

  it("fills the track for the busiest sign", () => {
    expect(densityLength(5, 5, 100)).toBe(100);
  });

  /// Square-rooted, so one talkative placement does not flatten the rest.
  it("grows as the root, not linearly", () => {
    expect(densityLength(1, 4, 100)).toBe(50);
    expect(densityLength(1, 4, 100)).toBeGreaterThan((1 / 4) * 100);
  });

  it("survives an empty reading without dividing by zero", () => {
    expect(Number.isFinite(densityLength(0, 0, 100))).toBe(true);
    expect(Number.isFinite(densityLength(1, 0, 100))).toBe(true);
  });
});

describe("where the pointer aims", () => {
  const c = chart();

  it("at a body, its own longitude", () => {
    expect(focusLongitude(c, "planet:sun")).toBe(10);
  });

  it("at a sign, its midpoint", () => {
    expect(focusLongitude(c, "sign:s0")).toBe(15);
    expect(focusLongitude(c, "sign:s6")).toBe(6 * 30 + 15);
  });

  it("at a house, the middle of its own sweep — not a fixed 15°", () => {
    // The first house spans 20°, so its midpoint is 10° past its cusp.
    expect(focusLongitude(c, "house:1")).toBe(10);
    // The third spans 50°.
    expect(focusLongitude(c, "house:3")).toBe(50 + 25);
  });

  it("at an aspect, the bisector of the SHORTER arc", () => {
    // Sun 10°, Moon 200° — 190° apart the long way, so the shorter arc runs
    // backwards and the bisector sits behind the Sun, not at 105°.
    const mid = focusLongitude(c, "aspect:sun-moon")!;
    expect(mid).toBe(10 + (((200 - 10 + 540) % 360) - 180) / 2);
    expect(mid).toBeLessThan(10);
  });

  it("bisects the plain case down the middle", () => {
    const near = chart({
      planets: [
        { id: "planet:sun", lon: 10 },
        { id: "planet:moon", lon: 70 },
      ],
      aspects: [{ id: "aspect:sun-moon", a: "planet:sun", b: "planet:moon" }],
    } as unknown as Partial<ChartData>);
    expect(focusLongitude(near, "aspect:sun-moon")).toBe(40);
  });

  it("is nothing for an element this chart does not have", () => {
    expect(focusLongitude(c, "planet:pluto")).toBeNull();
    expect(focusLongitude(c, "sign:nope")).toBeNull();
    expect(focusLongitude(c, "house:99")).toBeNull();
    expect(focusLongitude(c, "aspect:nope")).toBeNull();
    expect(focusLongitude(c, "nonsense")).toBeNull();
  });
});

describe("the chart angles", () => {
  it("are the two axes and their opposites", () => {
    const axes = axisLongitudes(chart());
    expect(axes.map((a) => a.label)).toEqual(["AC", "MC", "DC", "IC"]);
    expect(axes[0].lon).toBe(10);
    expect(axes[2].lon).toBe(190);
    expect(angularGap(axes[0].lon, axes[2].lon)).toBe(180);
    expect(angularGap(axes[1].lon, axes[3].lon)).toBe(180);
  });
});
