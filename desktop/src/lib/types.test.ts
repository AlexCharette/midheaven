import { describe, expect, it } from "vitest";
import { SIDEREAL, TROPICAL, calculationOf } from "./types";
import type { ChartData } from "./types";

const chart = (meta: Partial<ChartData["meta"]>): ChartData =>
  ({ meta: { name: "T", ...meta } }) as unknown as ChartData;

const fallback = { houseSystem: "whole-sign", ayanamsa: "lahiri" };

describe("the calculation a chart was computed with", () => {
  it("reads the codes the chart carries", () => {
    const c = chart({ house_system: "placidus", ayanamsa: "raman" });
    expect(calculationOf(c, fallback)).toEqual({
      houseSystem: "placidus",
      zodiac: SIDEREAL,
      ayanamsa: "raman",
    });
  });

  /// The contract sets an ayanamsa exactly when the chart is sidereal, and
  /// carries no zodiac code — so its presence is the code. This inference used
  /// to live inside an `$effect` in the reading view.
  it("infers the zodiac from whether an ayanamsa is set", () => {
    expect(calculationOf(chart({ house_system: "koch" }), fallback).zodiac).toBe(TROPICAL);
    expect(calculationOf(chart({ house_system: "koch", ayanamsa: "kp" }), fallback).zodiac).toBe(
      SIDEREAL,
    );
  });

  it("a tropical chart still shows an ayanamsa for the selector to sit on", () => {
    // The control is disabled for a tropical chart, but it has to show
    // something, and it should be what a switch to sidereal would use.
    expect(calculationOf(chart({ house_system: "koch" }), fallback).ayanamsa).toBe("lahiri");
  });

  /// `meta.house_system` is `#[serde(default)]` in the contract, so a chart
  /// saved before the field existed has an empty one.
  it("falls back only for a house system the chart does not state", () => {
    expect(calculationOf(chart({}), fallback).houseSystem).toBe("whole-sign");
    expect(calculationOf(chart({ house_system: "" }), fallback).houseSystem).toBe("whole-sign");
    expect(calculationOf(chart({ house_system: "equal" }), fallback).houseSystem).toBe("equal");
  });

  it("is a projection — the same chart always reads the same", () => {
    const c = chart({ house_system: "campanus", ayanamsa: "true-chitra" });
    expect(calculationOf(c, fallback)).toEqual(calculationOf(c, fallback));
  });
});
