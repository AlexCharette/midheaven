// IPC surface — the only place the frontend talks to the backend.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { BirthForm, ChartData, PlaceDto } from "./types";

export const searchPlaces = (query: string) =>
  invoke<PlaceDto[]>("search_places", { query });

export const build = (form: BirthForm) => invoke<ChartData>("build", { form });

export const saveArtifact = (path: string) =>
  invoke<string>("save_artifact", { path });

export const onTranscribeProgress = (handler: (pct: number) => void) =>
  listen<number>("transcribe-progress", (e) => handler(e.payload));

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
