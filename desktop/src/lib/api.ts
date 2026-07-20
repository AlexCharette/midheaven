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
