// Where saved readings are kept, and the one place that asks for it.
//
// The app picks no folder for anyone unasked, and asks nobody before the moment
// a folder is actually needed. That moment is when you decide to keep
// something — building a reading, or writing a note — and it is the same moment
// for a fresh install and for someone who has been running this for a year with
// nothing configured. One path, so neither case is a special case.
//
// The rule is separated from the asking for the usual reason: `whatToDo` is
// pure and directly testable, and everything below it is dialogs and disk.

import { open } from "@tauri-apps/plugin-dialog";
import { getPreferences, setPreferences } from "./api";

/** What to do about the readings folder. */
export type Verdict =
  /** One is configured; nothing to do. */
  | "haveIt"
  /** None is configured and we have not asked yet. */
  | "ask"
  /** None is configured and we already asked. */
  | "letItBe";

/**
 * `letItBe` is why this is a rule rather than an `if`. Declining is an answer:
 * asking again on the next reading, and the one after, would be nagging someone
 * who already said no — and the app works without a folder, so no is a position
 * it can respect rather than a state it has to escape.
 *
 * It is remembered for the run and nowhere else. A decline is not a preference,
 * so it is not written to one; the next launch may reasonably ask again.
 */
export function whatToDo(dir: string | null | undefined, askedAlready: boolean): Verdict {
  if (dir && dir.trim()) return "haveIt";
  return askedAlready ? "letItBe" : "ask";
}

let askedThisRun = false;

/** Forget that we asked — for tests, and for nothing else. */
export function forgetAsking() {
  askedThisRun = false;
}

/**
 * Ask for a readings folder and keep the answer. Always asks, so this is what
 * an explicit "choose a folder" is wired to — a person who just clicked that
 * has not declined anything.
 *
 * Returns the folder, or `null` if the dialog was dismissed.
 */
export async function chooseReadingsDir(): Promise<string | null> {
  askedThisRun = true;
  const picked = await open({ directory: true, title: "Choose a folder for saved readings" });
  if (typeof picked !== "string") return null;
  // Read-modify-write rather than sending a bare folder: `set_preferences`
  // takes the whole object, and anything not sent back would be erased.
  const prefs = await getPreferences();
  await setPreferences({ ...prefs, readings_dir: picked });
  return picked;
}

/**
 * The readings folder, asking for one the first time it turns out there is
 * none. `null` means the caller should carry on without a library — either
 * because the person declined just now, or because they declined earlier in
 * this run and are not being asked again.
 */
export async function ensureReadingsDir(): Promise<string | null> {
  const dir = (await getPreferences()).readings_dir;
  switch (whatToDo(dir, askedThisRun)) {
    case "haveIt":
      return dir!;
    case "letItBe":
      return null;
    case "ask":
      return chooseReadingsDir();
  }
}
