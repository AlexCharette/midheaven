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
  Chart,
  ChartData,
  ChartRow,
  ElementDto,
  JournalRow,
  LocaleDto,
  Note,
  OptionDto,
  PlaceDto,
  Person,
  Preferences,
  PreviewDto,
  PreviewInput,
  Query,
  StoredNote,
  View,
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
 * keeping its notes. `ayanamsa` is used only when zodiac is "sidereal". */
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

/** Reopen a chart from the store, by id. */
export const openChart = (id: string) => call<ChartData>("open_chart", { id });

/** Every chart the store holds, newest first, with who it is of and how much
 * has been written about it. */
export const listCharts = () => call<ChartRow[]>("list_charts");

/** Remove a chart and every note filed under it. The person stays. */
export const deleteChart = (id: string) => call<void>("delete_chart", { id });

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

/** A landed take's notes — the whole reading's, in reading order. */
export const stopRecording = () => call<Note[]>("stop_recording");

// ---- notes ----
//
// One path. These used to be two: four commands against the open chart's note
// list, and three more against a separate journal. Every one of them returns
// the reading's notes rather than a chart, because a chart no longer holds
// any.

/** Every note filed under the open reading's chart, in reading order. */
export const listNotes = () => call<Note[]>("list_notes");

/** Write a note against the open reading: new with no `id`, an amendment with
 * one. With no tags picked, the router files it from its own words. */
export const writeNote = (text: string, tags: string[], id?: string) =>
  call<Note[]>("write_note", { id, text, tags });

/** Merge a note into the one before it in reading order. */
export const mergeUp = (id: string) => call<Note[]>("merge_up", { id });

/** Remove a note. Final. */
export const deleteNote = (id: string) => call<void>("delete_note", { id });

/** Write a note against a sky nobody is consulting over — the home page's
 * path. The chart record is minted on the first note about that moment, so
 * every note over one sky shares one chart. */
export const writeMomentNote = (
  text: string,
  tags: string[],
  anchor: Chart,
  id?: string,
) => call<StoredNote>("write_moment_note", { id, text, tags, anchor });

/** Every note the app holds, whatever sky it was written over — newest
 * first. */
export const listJournal = () => call<JournalRow[]>("list_journal");

// ---- asking ----
//
// A closed set of questions rather than a search string with a syntax. The
// rows a query returns are the same rows `listJournal` returns, narrowed —
// searching does not produce a different kind of result.

/** Ask a question of the notes. An empty query is every note. */
export const queryNotes = (query: Query) => call<JournalRow[]>("query_notes", { query });

/** Save a question by name — new with no `id`, an amendment with one. */
export const writeView = (name: string, query: Query, id?: string) =>
  call<View>("write_view", { id, name, query });

export const listViews = () => call<View[]>("list_views");

export const deleteView = (id: string) => call<void>("delete_view", { id });

/** Everyone the store knows about — the people filter's options. */
export const listPeople = () => call<Person[]>("list_people");

/** Remove a person. Refused while any chart or note still refers to them. */
export const deletePerson = (id: string) => call<void>("delete_person", { id });

// ---- standing notes ----

/** The vocabulary as a plate you can click, with no chart behind it — the
 * tag picker for a note that is about no sky. Excludes `aspect:` pairs. */
export const listElements = () => call<ElementDto[]>("list_elements");

/** Write a note about no sky: craft knowledge about the elements, or an
 * observation about a person. Naming a person is what makes it the second. */
export const writeStandingNote = (
  text: string,
  tags: string[],
  person?: string,
  id?: string,
) => call<StoredNote>("write_standing_note", { id, text, tags, person });

export const getPreferences = () => call<Preferences>("get_preferences");

export const setPreferences = (prefs: Preferences) =>
  call<void>("set_preferences", { prefs });

export const listModels = (dir: string) => call<string[]>("list_models", { dir });

/** Open the bundled third-party license notices in the OS browser. */
export const openLicenses = () => call<void>("open_licenses");

export const artifactFilename = () => call<string>("artifact_filename");
