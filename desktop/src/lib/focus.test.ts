import { describe, expect, it } from "vitest";
import {
  NOTHING,
  cleared,
  focusedTag,
  matching,
  occupiedTags,
  passageCount,
  peeked,
  relatedTo,
  signDensity,
  toggled,
  touchesPins,
  unpeeked,
  visibleExcerpts,
  withMode,
} from "./focus";
import type { Focus } from "./focus";
import type { ChartData, Excerpt } from "./types";

/** A chart just real enough for the focus rules: two bodies with derived signs,
 * one aspect between them, and passages tagged from the vocabulary. */
function chart(excerpts: Excerpt[] = []): ChartData {
  return {
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
      { id: "planet:sun", glyph: "☉", name: "Sun", lon: 100, house: 4, sign: 3, deg: 10, min: 0 },
      { id: "planet:moon", glyph: "☽", name: "Moon", lon: 220, house: 8, sign: 7, deg: 10, min: 0 },
    ],
    aspects: [
      {
        id: "aspect:sun-moon",
        glyph: "△",
        name: "Trine",
        a: "planet:sun",
        b: "planet:moon",
        nature: "harmonious",
        orb: 0,
      },
    ],
    excerpts,
  } as unknown as ChartData;
}

const ex = (id: string, tags: string[]): Excerpt =>
  ({ id, time: "", span: [0, 0], text: id, tags }) as unknown as Excerpt;

describe("the focused element", () => {
  it("is nothing when nothing is pinned or hovered", () => {
    expect(focusedTag(NOTHING)).toBeNull();
  });

  it("follows the hover when nothing is pinned", () => {
    expect(focusedTag(peeked(NOTHING, "planet:sun"))).toBe("planet:sun");
    expect(focusedTag(unpeeked(peeked(NOTHING, "planet:sun")))).toBeNull();
  });

  it("is LOCKED by a pin — hovering no longer flips it", () => {
    const pinned = toggled(NOTHING, "planet:sun");
    expect(focusedTag(pinned)).toBe("planet:sun");
    // hovering something else while pinned changes nothing
    expect(focusedTag(peeked(pinned, "planet:moon"))).toBe("planet:sun");
  });

  it("goes to the most recent pin", () => {
    let f = toggled(NOTHING, "planet:sun");
    f = toggled(f, "planet:moon");
    expect(focusedTag(f)).toBe("planet:moon");
    // unpinning the newest hands the focus back to the older pin
    expect(focusedTag(toggled(f, "planet:moon"))).toBe("planet:sun");
  });

  it("returns to the hover once the last pin is cleared", () => {
    let f = peeked(NOTHING, "planet:moon");
    f = toggled(f, "planet:sun");
    expect(focusedTag(f)).toBe("planet:sun");
    expect(focusedTag(cleared(f))).toBe("planet:moon");
  });

  it("re-pinning an already-pinned tag moves it to the front of the focus", () => {
    let f = toggled(toggled(NOTHING, "planet:sun"), "planet:moon");
    f = toggled(f, "planet:moon"); // unpin
    f = toggled(f, "planet:moon"); // pin again
    expect(focusedTag(f)).toBe("planet:moon");
  });
});

describe("matching", () => {
  const c = chart([
    ex("x1", ["planet:sun", "sign:s3"]),
    ex("x2", ["planet:moon"]),
    ex("x3", []),
  ]);

  it("shows everything for an empty selection", () => {
    expect(matching(c, [], "any")).toHaveLength(3);
    expect(matching(c, [], "all")).toHaveLength(3);
  });

  it("any = the passage touches one of the tags", () => {
    expect(matching(c, ["planet:sun", "planet:moon"], "any").map((e) => e.id)).toEqual(["x1", "x2"]);
  });

  it("all = the passage touches every one", () => {
    expect(matching(c, ["planet:sun", "sign:s3"], "all").map((e) => e.id)).toEqual(["x1"]);
    expect(matching(c, ["planet:sun", "planet:moon"], "all")).toHaveLength(0);
  });

  it("counts the passages touching one tag", () => {
    expect(passageCount(c, "planet:sun")).toBe(1);
    expect(passageCount(c, "house:12")).toBe(0);
  });
});

describe("what the commentary shows", () => {
  const c = chart([
    ex("x1", ["planet:sun", "sign:s3"]),
    ex("x2", ["planet:moon"]),
    ex("x3", []),
  ]);

  it("is the whole reading when nothing is pinned or hovered", () => {
    expect(visibleExcerpts(c, NOTHING)).toHaveLength(3);
  });

  it("previews just the hovered element's passages while nothing is pinned", () => {
    const f = peeked(NOTHING, "planet:moon");
    expect(visibleExcerpts(c, f).map((e) => e.id)).toEqual(["x2"]);
  });

  it("stops previewing once anything is pinned — pins win over the hover", () => {
    let f = toggled(NOTHING, "planet:sun");
    f = peeked(f, "planet:moon");
    expect(visibleExcerpts(c, f).map((e) => e.id)).toEqual(["x1"]);
  });

  it("honours the filter mode across several pins", () => {
    let f = toggled(toggled(NOTHING, "planet:sun"), "sign:s3");
    expect(visibleExcerpts(c, f).map((e) => e.id)).toEqual(["x1"]);
    f = withMode(f, "all");
    expect(visibleExcerpts(c, f).map((e) => e.id)).toEqual(["x1"]);
    f = withMode(toggled(f, "planet:moon"), "all");
    expect(visibleExcerpts(c, f)).toHaveLength(0);
  });

  it("washes only the rows a pin touches, and nothing when nothing is pinned", () => {
    const f = toggled(NOTHING, "planet:sun");
    expect(touchesPins(f, c.excerpts[0])).toBe(true);
    expect(touchesPins(f, c.excerpts[1])).toBe(false);
    // a hover is not a pin
    expect(touchesPins(peeked(NOTHING, "planet:sun"), c.excerpts[0])).toBe(false);
  });
});

describe("what lights up with the focus", () => {
  const c = chart();

  it("a planet lights its sign, its house, its aspects and the far body", () => {
    expect(relatedTo(c, "planet:sun")).toEqual(
      new Set(["sign:s3", "house:4", "aspect:sun-moon", "planet:moon"]),
    );
  });

  it("is symmetric — a sign or house lights the bodies standing in it", () => {
    expect(relatedTo(c, "sign:s3")).toEqual(new Set(["planet:sun"]));
    expect(relatedTo(c, "house:8")).toEqual(new Set(["planet:moon"]));
    expect(relatedTo(c, "aspect:sun-moon")).toEqual(new Set(["planet:sun", "planet:moon"]));
  });

  it("never includes the focused tag itself", () => {
    for (const tag of ["planet:sun", "sign:s3", "house:4", "aspect:sun-moon"]) {
      expect(relatedTo(c, tag).has(tag), tag).toBe(false);
    }
  });

  it("lights nothing for an empty sign, house, or unknown tag", () => {
    expect(relatedTo(c, "sign:s11")).toEqual(new Set());
    expect(relatedTo(c, "house:12")).toEqual(new Set());
    expect(relatedTo(c, "planet:pluto")).toEqual(new Set());
    expect(relatedTo(c, "nonsense")).toEqual(new Set());
  });

  it("occupancy is the sign and house tags some body stands in", () => {
    expect(occupiedTags(c)).toEqual(new Set(["sign:s3", "house:4", "sign:s7", "house:8"]));
  });
});

describe("sign density", () => {
  it("counts a passage toward the sign it names", () => {
    const d = signDensity(chart([ex("x1", ["sign:s3"])]));
    expect(d[3]).toBe(1);
    expect(d.reduce((a, b) => a + b, 0)).toBe(1);
  });

  it("counts a passage toward the sign a tagged body stands in", () => {
    // The words never named the sign — the Sun standing in it carries the weight.
    const d = signDensity(chart([ex("x1", ["planet:sun"])]));
    expect(d[3]).toBe(1);
  });

  it("counts a passage once per sign however many of its tags point there", () => {
    // The sign and its tenant are the same hit, not two.
    const d = signDensity(chart([ex("x1", ["planet:sun", "sign:s3"])]));
    expect(d[3]).toBe(1);
  });

  it("spreads a passage across every sign it touches", () => {
    const d = signDensity(chart([ex("x1", ["planet:sun", "planet:moon"])]));
    expect(d[3]).toBe(1);
    expect(d[7]).toBe(1);
  });

  it("ignores tags that name no sign and no body", () => {
    const d = signDensity(chart([ex("x1", ["house:4", "aspect:sun-moon"])]));
    expect(d.reduce((a, b) => a + b, 0)).toBe(0);
  });

  it("is twelve long and all zero for a reading with no passages", () => {
    expect(signDensity(chart())).toEqual(new Array(12).fill(0));
  });
});

describe("the focus is a value", () => {
  it("NOTHING is the empty focus, which is what a reset restores", () => {
    // Pins and hover used to be cleared side by side at every call site, and a
    // missed half left the next reading lit up.
    expect(NOTHING.pinned).toHaveLength(0);
    expect(NOTHING.hovered).toBeNull();
    expect(focusedTag(NOTHING)).toBeNull();
  });

  it("every transition returns a new focus and leaves its input untouched", () => {
    const before: Focus = { pinned: ["planet:sun"], hovered: "planet:moon", mode: "any" };
    const snapshot = JSON.stringify(before);
    for (const after of [
      toggled(before, "sign:s3"),
      toggled(before, "planet:sun"),
      cleared(before),
      peeked(before, "house:4"),
      unpeeked(before),
      withMode(before, "all"),
    ]) {
      expect(after).not.toBe(before);
      expect(JSON.stringify(before), "the input was mutated").toBe(snapshot);
    }
  });
});
