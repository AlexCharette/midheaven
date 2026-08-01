// IPC surface — the only place the frontend talks to the backend.
//
// Every command goes through `call`, for two reasons.
//
// The name is a `CommandName`, generated from the list the backend registers, so
// a command renamed in Rust is a TypeScript error here rather than a rejection at
// runtime. (Argument names are still matched by hand against each command's
// parameters — that half is not yet checked.)
//
// And a failure arrives as one shape. Tauri rejects a `Result<_, String>` command
// with the bare string it returned, which every caller used to stringify for
// itself: `String(e)` in five components, `` `${e}` `` in nine more, with no
// stated rule about which failures become a toast and which a local field. Now
// `call` throws a `CommandFailed` carrying the command it came from, and callers
// read the message through `reason`.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { reason } from "./failure";
import type { CommandName } from "./generated/commands";
import type {
  AppChrome,
  BirthForm,
  CalculationDefaults,
  ChartData,
  LocaleDto,
  OptionDto,
  PlaceDto,
  Preferences,
  PreviewDto,
  PreviewInput,
  ReadingEntry,
} from "./types";

/** A command the backend refused. `message` is the reason it gave, ready to
 * show — the backend writes these for the astrologer, not for a log. */
export class CommandFailed extends Error {
  constructor(
    readonly command: CommandName,
    message: string,
  ) {
    super(message);
    this.name = "CommandFailed";
  }
}

async function call<T>(command: CommandName, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw new CommandFailed(command, reason(e));
  }
}

export const searchPlaces = (query: string) =>
  call<PlaceDto[]>("search_places", { query });

export const listLocales = () => call<LocaleDto[]>("list_locales");

export const listHouseSystems = () => call<OptionDto[]>("list_house_systems");

export const listAyanamsas = () => call<OptionDto[]>("list_ayanamsas");

/** The calculation a form starts from when nothing has been chosen — served
 * rather than restated, so `chart::systems::DEFAULTS` is the only place the
 * three codes are written. */
export const calculationDefaults = () =>
  call<CalculationDefaults>("calculation_defaults");

/** The app's own window furniture, in the person's language — the reading
 * view's share of it; the forms are still English in their components. */
export const appChrome = () => call<AppChrome>("app_chrome");

export const build = (form: BirthForm) => call<ChartData>("build", { form });

/** Recompute the current chart's geometry under a new house system / zodiac,
 * keeping its passages. `ayanamsa` is used only when zodiac is "sidereal". */
export const reproject = (houseSystem: string, zodiac: string, ayanamsa: string | null) =>
  call<ChartData>("reproject", { houseSystem, zodiac, ayanamsa });

/** Chart geometry for an arbitrary moment — the live calculator's engine.
 * Side-effect-free on the backend, so it may be called at scrub rates;
 * non-fatal warnings (DST fold) come back inline, never as toast events. */
export const preview = (input: PreviewInput) => call<PreviewDto>("preview", { input });

/** The calculator's opening place: last used if it still resolves, else a
 * default city. Total — never fails. */
export const lastPlace = () => call<PlaceDto>("last_place");

/** Persist the calculator's place; called on pick, never per scrub tick. */
export const setLastPlace = (id: number) => call<void>("set_last_place", { id });

export const loadChart = (path: string) => call<ChartData>("load_chart", { path });

export const listReadings = () => call<ReadingEntry[]>("list_readings");

export const deleteReading = (dir: string) => call<void>("delete_reading", { dir });

export const saveArtifact = (path: string) => call<string>("save_artifact", { path });

export const savePdf = (path: string) => call<string>("save_pdf", { path });

export const onTranscribeProgress = (handler: (pct: number) => void) =>
  listen<number>("transcribe-progress", (e) => handler(e.payload));

/** Non-fatal build/routing warnings the backend used to write to stderr
 * (DST-ambiguous birth time, Verify-gate rejections). */
export const onBuildWarnings = (handler: (warnings: string[]) => void) =>
  listen<string[]>("build-warnings", (e) => handler(e.payload));

export const startRecording = (model: string) =>
  call<void>("start_recording", { model });

export const stopRecording = () => call<ChartData>("stop_recording");

export const mergeUp = (id: string) => call<ChartData>("merge_up", { id });

export const correctExcerpt = (id: string, text: string) =>
  call<ChartData>("correct_excerpt", { id, text });

export const addExcerpt = (text: string, tags: string[]) =>
  call<ChartData>("add_excerpt", { text, tags });

export const deleteExcerpt = (id: string) => call<ChartData>("delete_excerpt", { id });

export const getPreferences = () => call<Preferences>("get_preferences");

export const setPreferences = (prefs: Preferences) =>
  call<void>("set_preferences", { prefs });

export const listModels = (dir: string) => call<string[]>("list_models", { dir });

/** Open the bundled third-party license notices in the OS browser. */
export const openLicenses = () => call<void>("open_licenses");

export const artifactFilename = () => call<string>("artifact_filename");
