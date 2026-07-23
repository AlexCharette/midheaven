// IPC surface — the only place the frontend talks to the backend.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { BirthForm, ChartData, LocaleDto, PlaceDto, Preferences, ReadingEntry } from "./types";

export const searchPlaces = (query: string) =>
  invoke<PlaceDto[]>("search_places", { query });

export const listLocales = () => invoke<LocaleDto[]>("list_locales");

export const build = (form: BirthForm) => invoke<ChartData>("build", { form });

export const loadChart = (path: string) => invoke<ChartData>("load_chart", { path });

export const listReadings = () => invoke<ReadingEntry[]>("list_readings");

export const deleteReading = (dir: string) => invoke<void>("delete_reading", { dir });

export const saveArtifact = (path: string) =>
  invoke<string>("save_artifact", { path });

export const savePdf = (path: string) => invoke<string>("save_pdf", { path });

export const onTranscribeProgress = (handler: (pct: number) => void) =>
  listen<number>("transcribe-progress", (e) => handler(e.payload));

/** Non-fatal build/routing warnings the backend used to write to stderr
 * (DST-ambiguous birth time, Verify-gate rejections). */
export const onBuildWarnings = (handler: (warnings: string[]) => void) =>
  listen<string[]>("build-warnings", (e) => handler(e.payload));

export const startRecording = (model: string) =>
  invoke<void>("start_recording", { model });

export const stopRecording = () => invoke<ChartData>("stop_recording");

export const mergeUp = (id: string) => invoke<ChartData>("merge_up", { id });

export const correctExcerpt = (id: string, text: string) =>
  invoke<ChartData>("correct_excerpt", { id, text });

export const addExcerpt = (text: string, tags: string[]) =>
  invoke<ChartData>("add_excerpt", { text, tags });

export const deleteExcerpt = (id: string) =>
  invoke<ChartData>("delete_excerpt", { id });

export const getPreferences = () => invoke<Preferences>("get_preferences");

export const setPreferences = (prefs: Preferences) =>
  invoke<void>("set_preferences", { prefs });

export const listModels = (dir: string) => invoke<string[]>("list_models", { dir });

export const artifactFilename = () => invoke<string>("artifact_filename");
