import { describe, expect, it } from "vitest";
import { anchorFor, matches, rowSubject, search } from "./journal";
import type { ChartData, JournalRow, PreviewInput } from "./types";

const input: PreviewInput = {
  date: "2026-08-04",
  time: "09:12",
  place_id: 2950159,
  lang: "en",
  // What the calculator actually sends when nothing has been chosen: blank
  // means "not stated", and the backend's ladder answers it.
  house_system: "",
  zodiac: "",
  ayanamsa: null,
};

const chart = (over: Partial<ChartData["meta"]> = {}) =>
  ({
    meta: {
      name: "",
      born: "2026-08-04 09:12",
      place: "Berlin, Germany",
      system: "Whole Sign",
      zodiac: "Tropical",
      house_system: "whole-sign",
      locale: "en",
      ...over,
    },
  }) as unknown as ChartData;

describe("what a note is about", () => {
  /** The calculator's codes may be blank — "not chosen", answered by the
   * backend's default ladder. Recording the blank would store "unstated" and
   * let the note drift the day a preference changes. */
  it("records what the blanks resolved to, not the blanks", () => {
    const a = anchorFor(input, chart());
    expect(a.houseSystem).toBe("whole-sign");
    expect(a.locale).toBe("en");
    expect(a.kind).toBe("moment");
  });

  it("keeps the place id, which only the request knows", () => {
    // A preview does not stamp `meta.birth`, so the chart cannot say which
    // gazetteer entry this was.
    expect(anchorFor(input, chart()).seed).toEqual({
      place_id: 2950159,
      date: "2026-08-04",
      time: "09:12",
    });
  });

  it("carries an ayanamsa only when the chart is sidereal", () => {
    expect(anchorFor(input, chart()).ayanamsa).toBeUndefined();
    expect(anchorFor(input, chart({ ayanamsa: "lahiri" })).ayanamsa).toBe("lahiri");
  });

  it("names a subject only when there is one", () => {
    expect(anchorFor(input, chart()).reading).toBeUndefined();
    const about = { id: "r1", name: "Mira Holt", born: "1990-07-13 14:30" };
    expect(anchorFor(input, chart(), about).reading).toEqual(about);
  });
});

const row = (over: Partial<JournalRow> = {}): JournalRow =>
  ({
    writtenAt: "2026-08-04T09:12:00+02:00",
    text: "Mars is loud today.",
    tags: ["planet:mars"],
    tagLabels: ["Mars"],
    spoken: false,
    source: {
      kind: "journal",
      id: "abc",
      anchor: {
        kind: "moment",
        seed: { place_id: 1, date: "2026-08-04", time: "09:12" },
        place: "Berlin, Germany",
        houseSystem: "whole-sign",
        locale: "en",
      },
    },
    ...over,
  }) as JournalRow;

const aboutMira = row({
  text: "She kept returning to the tenth house.",
  tags: ["house:10"],
  tagLabels: ["Tenth House"],
  spoken: true,
  source: {
    kind: "reading",
    noteId: "x1",
    chartPath: "/lib/mira/chart.json",
    dir: "/lib/mira",
    name: "Mira Holt",
    born: "1990-07-13 14:30",
  },
});

describe("finding a note again", () => {
  it("names a reading by its person and a moment by its place", () => {
    expect(rowSubject(aboutMira)).toBe("Mira Holt");
    expect(rowSubject(row())).toBe("Berlin, Germany");
  });

  it("shows everything when nothing is typed", () => {
    expect(search([row(), aboutMira], "  ")).toHaveLength(2);
  });

  /** The three ways anyone reaches for a note, through one field — making a
   * person say which kind of thing they are typing would be making them work. */
  it("finds a note by its words, its person, or its elements", () => {
    const rows = [row(), aboutMira];
    expect(search(rows, "loud").map((r) => r.text)).toEqual(["Mars is loud today."]);
    expect(search(rows, "mira")).toEqual([aboutMira]);
    expect(search(rows, "tenth house")).toEqual([aboutMira]);
    expect(search(rows, "berlin")).toEqual([row()]);
  });

  /** Nobody searches for `planet:mars`, and a note written under a Russian
   * reading carries Russian labels so it answers to Russian. */
  it("matches the element's name, not its id", () => {
    expect(matches(row(), "Mars")).toBe(true);
    expect(matches(row({ tagLabels: ["Марс"] }), "марс")).toBe(true);
    expect(matches(row(), "planet:mars")).toBe(false);
  });

  it("ignores case and surrounding space", () => {
    expect(matches(row(), "  LOUD ")).toBe(true);
  });

  it("finds nothing when nothing matches", () => {
    expect(search([row(), aboutMira], "saturn")).toEqual([]);
  });
});
