import { describe, expect, it } from "vitest";
import {
  MINUTES_PER_DAY,
  addMonths,
  daysInYear,
  fromDays,
  fromMinutes,
  parseDate,
  parseTime,
  ringAngles,
  ringDetent,
  ringDragMinutes,
  ringStep,
  setYear,
  toDays,
  toMinutes,
} from "./civil";

describe("day-number round trips", () => {
  it("agrees with the Unix epoch anchor", () => {
    expect(toDays(1970, 1, 1)).toBe(719468);
    expect(fromDays(719468)).toEqual({ y: 1970, m: 1, d: 1 });
  });

  it("round-trips across leap and century boundaries", () => {
    for (const [y, m, d] of [
      [2024, 2, 29], [2000, 2, 29], [1900, 2, 28], [1994, 3, 21],
      [1600, 12, 31], [2100, 1, 1],
    ] as const) {
      expect(fromDays(toDays(y, m, d))).toEqual({ y, m, d });
    }
  });

  it("counts leap days where the Gregorian rules say", () => {
    expect(daysInYear(2024)).toBe(366);
    expect(daysInYear(2023)).toBe(365);
    expect(daysInYear(1900)).toBe(365); // century, not divisible by 400
    expect(daysInYear(2000)).toBe(366);
  });
});

describe("parsing", () => {
  it("rejects calendar-impossible dates, accepts real ones", () => {
    expect(parseDate("2024-02-30")).toBeNull();
    expect(parseDate("2023-02-29")).toBeNull();
    expect(parseDate("2024-13-01")).toBeNull();
    expect(parseDate("2024-00-10")).toBeNull();
    expect(parseDate("not-a-date")).toBeNull();
    expect(parseDate("2024-02-29")).toEqual({ y: 2024, m: 2, d: 29 });
  });

  it("gates times to a 24h clock", () => {
    expect(parseTime("24:00")).toBeNull();
    expect(parseTime("14:60")).toBeNull();
    expect(parseTime("14:32")).toBe(14 * 60 + 32);
    expect(parseTime("0:05")).toBe(5);
  });
});

describe("the canonical instant", () => {
  it("round-trips date+time through minutes", () => {
    const min = toMinutes("1994-03-21", "14:32")!;
    expect(fromMinutes(min)).toEqual({ date: "1994-03-21", time: "14:32" });
  });

  it("carries across midnight in both directions", () => {
    const late = toMinutes("2024-12-31", "23:50")!;
    expect(fromMinutes(late + 20)).toEqual({ date: "2025-01-01", time: "00:10" });
    expect(fromMinutes(toMinutes("2025-01-01", "00:10")! - 20)).toEqual({
      date: "2024-12-31",
      time: "23:50",
    });
  });

  it("crosses Feb 28→29 only in leap years", () => {
    expect(fromMinutes(toMinutes("2024-02-28", "23:59")! + 1).date).toBe("2024-02-29");
    expect(fromMinutes(toMinutes("2023-02-28", "23:59")! + 1).date).toBe("2023-03-01");
  });
});

describe("setYear", () => {
  it("keeps month/day and clamps Feb 29 in common years", () => {
    expect(setYear("1994-03-21", 2001)).toBe("2001-03-21");
    expect(setYear("2024-02-29", 2023)).toBe("2023-02-28");
    expect(setYear("2024-02-29", 2028)).toBe("2028-02-29");
  });
});

describe("addMonths", () => {
  it("keeps day and time, clamping to the target month's length", () => {
    const jan31 = toMinutes("2023-01-31", "14:32")!;
    expect(fromMinutes(addMonths(jan31, 1))).toEqual({ date: "2023-02-28", time: "14:32" });
    expect(fromMinutes(addMonths(jan31, 12))).toEqual({ date: "2024-01-31", time: "14:32" });
    expect(fromMinutes(addMonths(jan31, -2))).toEqual({ date: "2022-11-30", time: "14:32" });
  });

  it("rolls the year across December/January", () => {
    const dec = toMinutes("2023-12-15", "08:00")!;
    expect(fromMinutes(addMonths(dec, 1)).date).toBe("2024-01-15");
    expect(fromMinutes(addMonths(toMinutes("2024-01-15", "08:00")!, -1)).date).toBe("2023-12-15");
  });
});

describe("ringAngles", () => {
  it("turns the time ring once per day", () => {
    expect(ringAngles(toMinutes("2023-06-15", "00:00")!).timeAngle).toBe(0);
    expect(ringAngles(toMinutes("2023-06-15", "12:00")!).timeAngle).toBe(180);
    expect(ringAngles(toMinutes("2023-06-15", "18:00")!).timeAngle).toBe(270);
  });

  it("gears the date ring to the (leap-aware) year", () => {
    // Jan 1 00:00 sits on the index; July 2 12:00 of a common year is halfway
    // through its 365 days (182.5 days elapsed).
    expect(ringAngles(toMinutes("2023-01-01", "00:00")!).dateAngle).toBe(0);
    const half = ringAngles(toMinutes("2023-07-02", "12:00")!).dateAngle;
    expect(half).toBeCloseTo(180, 6);
    // The last minute of the year approaches — but never reaches — 360°.
    const end = ringAngles(toMinutes("2023-12-31", "23:59")!).dateAngle;
    expect(end).toBeGreaterThan(359.9);
    expect(end).toBeLessThan(360);
  });

  it("creeps the date ring as the time ring turns (continuous gearing)", () => {
    const midnight = ringAngles(toMinutes("2023-06-15", "00:00")!).dateAngle;
    const noon = ringAngles(toMinutes("2023-06-15", "12:00")!).dateAngle;
    expect(noon - midnight).toBeCloseTo(0.5 / 365 * 360, 9);
  });

  it("stays finite over an unbounded scrub", () => {
    const min = toMinutes("2023-06-15", "12:00")! + 10 * 366 * MINUTES_PER_DAY;
    const a = ringAngles(min);
    expect(a.timeAngle).toBeGreaterThanOrEqual(0);
    expect(a.timeAngle).toBeLessThan(360);
    expect(a.dateAngle).toBeGreaterThanOrEqual(0);
    expect(a.dateAngle).toBeLessThan(360);
  });
});

describe("the instrument rings", () => {
  /// `ringAngles` has been tested since it was written; the transform a drag
  /// actually uses is its inverse, and lived in a closure inside `TimeRings`.
  it("a drag and the ring rotation are inverses", () => {
    const days = daysInYear(1990);
    for (const minutes of [5, 60, 725, -300, MINUTES_PER_DAY / 4]) {
      // time ring: one turn per day
      const deg = -(minutes / MINUTES_PER_DAY) * 360;
      expect(ringDragMinutes("time", deg, days)).toBeCloseTo(minutes, 9);
      // date ring: one turn per year
      const dateDeg = -(minutes / (days * MINUTES_PER_DAY)) * 360;
      expect(ringDragMinutes("date", dateDeg, days)).toBeCloseTo(minutes, 9);
    }
  });

  it("the ring follows the finger — a clockwise drag winds the moment back", () => {
    expect(ringDragMinutes("time", 90, 365)).toBeLessThan(0);
    expect(ringDragMinutes("date", 90, 365)).toBeLessThan(0);
  });

  it("the date ring is geared to the year's own length", () => {
    // A leap year turns slower: the same drag moves fewer of its longer year.
    expect(ringDragMinutes("date", -90, 366)).toBeGreaterThan(ringDragMinutes("date", -90, 365));
  });

  it("the date ring detents to whole days, the time ring does not", () => {
    const start = toMinutes("1990-07-13", "14:30")!;
    // Half a day of drag is no day at all on the date ring.
    expect(ringDetent("date", start, MINUTES_PER_DAY * 0.4)).toBe(start);
    expect(ringDetent("date", start, MINUTES_PER_DAY * 0.6)).toBe(start + MINUTES_PER_DAY);
    // …so the time of day never shifts.
    expect(fromMinutes(ringDetent("date", start, MINUTES_PER_DAY * 2.3)).time).toBe("14:30");
    // The time ring is continuous, and rolling past midnight moves the date.
    expect(ringDetent("time", start, 90)).toBe(start + 90);
    expect(fromMinutes(ringDetent("time", start, MINUTES_PER_DAY)).date).toBe("1990-07-14");
  });

  it("an arrow key steps by the ring's own unit, shift coarsens it", () => {
    const start = toMinutes("1990-07-13", "14:30")!;
    expect(ringStep("time", start, 1, false)).toBe(start + 5);
    expect(ringStep("time", start, -1, false)).toBe(start - 5);
    expect(ringStep("time", start, 1, true)).toBe(start + 60);
    expect(ringStep("date", start, 1, false)).toBe(start + MINUTES_PER_DAY);
    // A month is not a fixed number of minutes.
    expect(fromMinutes(ringStep("date", start, 1, true)).date).toBe("1990-08-13");
    expect(fromMinutes(ringStep("date", start, -1, true)).date).toBe("1990-06-13");
  });

  it("a coarse date step clamps a day that the next month lacks", () => {
    const jan31 = toMinutes("1990-01-31", "09:00")!;
    expect(fromMinutes(ringStep("date", jan31, 1, true)).date).toBe("1990-02-28");
    expect(fromMinutes(ringStep("date", jan31, 1, true)).time).toBe("09:00");
  });
});
