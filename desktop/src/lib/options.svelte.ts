// What this install offers: the selector lists the backend owns, and whether a
// transcription model is installed.
//
// All of it comes from the backend rather than being re-encoded here — the
// languages and their endonyms from `i18n::Locale`, the house systems and
// ayanamsas from `chart::systems` — so adding one on that side needs no change
// on this one.

import { calculationDefaults, listAyanamsas, listHouseSystems, listLocales } from "./api";
import { reason } from "./failure";
import { notify } from "./toasts.svelte";
import type { CalculationDefaults, LocaleDto, OptionDto } from "./types";

/** Reading languages offered in the UI (`list_locales`): endonym labels for the
 * selectors and the house-name suffix to strip. Empty until `loadLocales`. */
export const locales = $state<LocaleDto[]>([]);

/** House systems and ayanamsas offered in the form, each `{code,label}`
 * (`list_house_systems` / `list_ayanamsas`). Empty until `loadCalcOptions`. */
export const houseSystems = $state<OptionDto[]>([]);
export const ayanamsas = $state<OptionDto[]>([]);

/** Populate `locales` from the backend once; a no-op after the first call. */
export async function loadLocales() {
  if (locales.length > 0) return;
  try {
    locales.push(...(await listLocales()));
  } catch (e) {
    notify(reason(e), "error");
  }
}

/** The calculation a form starts from when nothing has been chosen.
 *
 * Served by the backend rather than restated here: `chart::systems::DEFAULTS` is
 * the only place the three codes are written. Before `loadCalcOptions` answers
 * they are *blank*, not guessed — a blank code means "not stated", which is what
 * the backend's ladder already does with one (`Codes::new` filters empties, and
 * `a_blank_choice_is_no_choice` pins it). So the first frame asks for the
 * backend's defaults by saying nothing, rather than by naming them and hoping
 * the two agree. */
const unstated: CalculationDefaults = { houseSystem: "", zodiac: "", ayanamsa: "" };
const calcDefaults = $state({ value: unstated });

export const defaults = (): CalculationDefaults => calcDefaults.value;

/** Populate the calculation-option lists and defaults once; a no-op after the
 * first call. */
export async function loadCalcOptions() {
  try {
    if (houseSystems.length === 0) houseSystems.push(...(await listHouseSystems()));
    if (ayanamsas.length === 0) ayanamsas.push(...(await listAyanamsas()));
    calcDefaults.value = await calculationDefaults();
  } catch (e) {
    notify(reason(e), "error");
  }
}

/** The word to strip from a house name for `code` (" House", " дом"), or ""
 * when the locale list hasn't loaded or the code is unknown. The one client
 * home for the mapping — the strings themselves come from the backend. */
export function houseSuffix(code: string): string {
  return locales.find((l) => l.code === code)?.houseSuffix ?? "";
}

/** The whisper-model path from preferences. A non-empty value is what enables
 * live recording, so it belongs with the rest of "what this install can do"
 * rather than with the state of the reading. */
const model = $state({ path: "" });

export const modelPath = (): string => model.path;
export const canRecord = (): boolean => model.path !== "";

export function setModelPath(path: string) {
  model.path = path;
}
