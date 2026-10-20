import { describe, expect, it } from "vitest";
import { whatToDo } from "./readings";

describe("when to ask for a readings folder", () => {
  it("does not ask when one is configured", () => {
    expect(whatToDo("/Users/x/Readings", false)).toBe("haveIt");
    expect(whatToDo("/Users/x/Readings", true)).toBe("haveIt");
  });

  it("asks the first time there is none", () => {
    expect(whatToDo(null, false)).toBe("ask");
    expect(whatToDo(undefined, false)).toBe("ask");
  });

  /** Declining is an answer. Asking again on the next reading, and the one
   * after, would be nagging someone who already said no — and the app works
   * without a folder, so no is a position it can respect. */
  it("takes no for an answer, for the rest of the run", () => {
    expect(whatToDo(null, true)).toBe("letItBe");
  });

  /** `set_preferences` normalizes a blank folder to null, but a hand-edited
   * preferences.json is a file a person can write. Blank means unset. */
  it("treats a blank folder as no folder", () => {
    expect(whatToDo("", false)).toBe("ask");
    expect(whatToDo("   ", false)).toBe("ask");
    expect(whatToDo("   ", true)).toBe("letItBe");
  });
});
