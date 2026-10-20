import { describe, expect, it } from "vitest";
import { anchorFor, matches, natalOf, previewOf, rowMark, rowSubject, search, zoomOnto } from "./journal";
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

  it("names a person only when there is one", () => {
    expect(anchorFor(input, chart()).person).toBeUndefined();
    expect(anchorFor(input, chart(), "p1").person).toBe("p1");
  });
});

const row = (over: Partial<JournalRow> = {}): JournalRow =>
  ({
    id: "abc",
    writtenAt: "2026-08-04T09:12:00+02:00",
    text: "Mars is loud today.",
    tags: ["planet:mars"],
    tagLabels: ["Mars"],
    anchor: { kind: "chart", id: "c1" },
    chart: {
      id: "c1",
      kind: "moment",
      seed: { place_id: 1, date: "2026-08-04", time: "09:12" },
      place: "Berlin, Germany",
      houseSystem: "whole-sign",
      locale: "en",
    },
    ...over,
  }) as JournalRow;

const aboutMira = row({
  id: "x1",
  anchor: { kind: "chart", id: "c1" },
  text: "She kept returning to the tenth house.",
  tags: ["house:10"],
  tagLabels: ["Tenth House"],
  person: { id: "p1", name: "Mira Holt", born: "1990-07-13 14:30" },
});

const standing = row({
  id: "s1",
  anchor: { kind: "standing" },
  chart: undefined,
  text: "Leo wants to be seen doing the thing.",
  tags: ["sign:leo"],
  tagLabels: ["Leo"],
});

const aboutAPerson = row({
  id: "p9",
  anchor: { kind: "person", id: "p1" },
  chart: undefined,
  text: "She keeps deflecting about her father.",
  person: { id: "p1", name: "Mira Holt", born: "1990-07-13 14:30" },
});

describe("what a row is about", () => {
  /** A standing note is about its elements, and the chips already say so —
   * repeating it in the subject slot would be saying it twice. */
  it("says nothing for a standing note and lets the chips carry it", () => {
    expect(rowSubject(standing)).toBe("");
    expect(standing.tagLabels).toEqual(["Leo"]);
  });

  it("names the person for a note about one, with no sky to fall back on", () => {
    expect(rowSubject(aboutAPerson)).toBe("Mira Holt");
  });

  /** A mark rather than a colour: the palette is CVD-validated and colour is
   * never the only signal. */
  it("marks the three kinds distinctly", () => {
    const marks = [row(), aboutAPerson, standing].map((r) => rowMark(r).glyph);
    expect(new Set(marks).size).toBe(3);
    expect(marks.every((m) => m.length > 0)).toBe(true);
  });

  /** The search box reaches a standing note through its words and its
   * elements — it has no person or place to be found by. */
  it("finds a standing note by its words or its elements", () => {
    expect(search([standing], "seen")).toHaveLength(1);
    expect(search([standing], "leo")).toHaveLength(1);
    expect(search([standing], "berlin")).toHaveLength(0);
  });
});

describe("finding a note again", () => {
  it("names a chart by its person, and one with no person by its place", () => {
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

describe("natalOf", () => {
  const seed = { place_id: 1, date: "1988-04-17", time: "06:42" };
  const chart = (id: string, kind: string, person?: string) => ({
    chart: { id, kind, seed, place: "Berlin", houseSystem: "whole-sign", locale: "en", person },
    notes: 0,
  });
  it("prefers the person's natal chart", () => {
    const rows = [chart("m", "moment", "p1"), chart("n", "natal", "p1"), chart("x", "natal", "p2")];
    expect(natalOf("p1", rows)?.id).toBe("n");
  });
  it("falls back to any chart of theirs, then to none", () => {
    expect(natalOf("p1", [chart("m", "moment", "p1")])?.id).toBe("m");
    expect(natalOf("p3", [chart("m", "moment", "p1")])).toBeNull();
  });
});

describe("previewOf", () => {
  it("reads a sidereal chart's zodiac off its ayanamsa", () => {
    const seed = { place_id: 7, date: "1979-11-02", time: "22:15" };
    const base = { id: "c", kind: "natal", seed, place: "Lisbon", houseSystem: "placidus", locale: "ru" };
    expect(previewOf(base)).toEqual({
      date: "1979-11-02",
      time: "22:15",
      place_id: 7,
      lang: "ru",
      house_system: "placidus",
      zodiac: "tropical",
      ayanamsa: null,
    });
    expect(previewOf({ ...base, ayanamsa: "lahiri" }).zodiac).toBe("sidereal");
  });
});

describe("zoomOnto", () => {
  const centre = { x: 400, y: 400 };
  const stage = { w: 1000, h: 800 };
  it("sends the notes outward, past the rim the element faces", () => {
    expect(zoomOnto({ x: 100, y: 400 }, centre, stage, 2, 80).side).toBe("left");
    expect(zoomOnto({ x: 700, y: 400 }, centre, stage, 2, 80).side).toBe("right");
  });
  it("sends a hub element's notes right", () => {
    expect(zoomOnto({ x: 390, y: 420 }, centre, stage, 2, 80).side).toBe("right");
  });
  it("carries the element to its resting point", () => {
    const z = zoomOnto({ x: 100, y: 300 }, centre, stage, 2, 80);
    expect(100 * 2 + z.tx).toBeCloseTo(z.rest.x);
    expect(300 * 2 + z.ty).toBeCloseTo(z.rest.y);
    expect(z.rest).toEqual({ x: 640, y: 400 });
  });
});
