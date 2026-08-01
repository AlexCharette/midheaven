import { describe, expect, it } from "vitest";
import { fmt, plural } from "./chrome.svelte";

describe("filling a chrome format string", () => {
  it("replaces each placeholder by name", () => {
    expect(fmt("{shown} of {total} passages", { shown: 3, total: 5 })).toBe("3 of 5 passages");
    expect(fmt("· {n} more", { n: 4 })).toBe("· 4 more");
  });

  it("leaves a string with no placeholders alone", () => {
    expect(fmt("Commentary", {})).toBe("Commentary");
  });
});

describe("a count takes the form its number calls for", () => {
  const en = { one: "{n} passage", other: "{n} passages" };

  /// The regression this exists to prevent: collapsing four `n === 1` ternaries
  /// onto a single un-inflected string made the hub read-out say "1 passages".
  it("one is singular, everything else is plural", () => {
    expect(plural(en, 1)).toBe("1 passage");
    expect(plural(en, 2)).toBe("2 passages");
    expect(plural(en, 0)).toBe("0 passages");
    expect(plural(en, 11)).toBe("11 passages");
  });

  it("a language that does not inflect says so by giving both forms alike", () => {
    const ru = { one: "{n} фрагментов", other: "{n} фрагментов" };
    expect(plural(ru, 1)).toBe("1 фрагментов");
    expect(plural(ru, 5)).toBe("5 фрагментов");
  });
});
