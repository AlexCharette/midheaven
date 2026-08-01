// The app's own words, in the person's language.
//
// The core has localized element names since the beginning; the PDF and the
// artifact have carried chrome tables for a while. The window that produces
// both was English-only, with eleven of these strings already written in
// `ArtifactChrome` — one of them character-for-character.
//
// This covers the reading view. The forms and the preferences panel are still
// English literals in their components: moving those means writing Russian copy
// that does not exist yet, which is authoring rather than refactoring.

import { appChrome } from "./api";
import { reason } from "./failure";
import { notify } from "./toasts.svelte";
import type { AppChrome } from "./types";

/** What the window says before the backend answers. English, because that is
 * what it said before this module existed. */
const fallback: AppChrome = {
  indexOfElements: "Index of Elements",
  bands: ["planets", "signs", "houses", "aspects"],
  commentary: "Commentary",
  passagesTouching: "passages touching",
  any: "any",
  all: "all",
  anyTitle: "Passages touching any selected element",
  allTitle: "Only passages touching every selected element",
  ofSelection: "of the selection ·",
  clear: "clear",
  count: "{shown} of {total} passages",
  passages: "{n} passages",
  fewer: "· fewer",
  more: "· {n} more",
  wheelAria: "Natal chart wheel; the index of elements offers the same filters",
  emptyNoneRouted: "No passages are filed under this chart yet.",
  emptyNoMatch: "No passage touches {word} of the selected elements.",
};

const store = $state({ value: fallback });

/** The app's words. */
export const t = (): AppChrome => store.value;

/** Fill a chrome format string. `{name}` is replaced by `vals.name` — the same
 * shape the artifact's viewer uses, because they come from the same tables. */
export const fmt = (template: string, vals: Record<string, string | number>): string =>
  template.replace(/\{(\w+)\}/g, (_, k) => String(vals[k]));

/** Load the chrome for the configured language. A no-op to call twice. */
export async function loadChrome() {
  try {
    store.value = await appChrome();
  } catch (e) {
    notify(reason(e), "error");
  }
}
