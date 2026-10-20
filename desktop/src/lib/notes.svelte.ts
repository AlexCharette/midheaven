// The open reading's notes.
//
// Separate from `session.svelte.ts` because the session is a state machine
// about *phases* — idle, open, recording, transcribing — and notes are not a
// phase. They used to ride on the chart, so the session carried them by
// accident; now that a chart is a sky and nothing else, they need a home of
// their own, and this is the smallest one that is still a single source.
//
// Every note command returns the whole list rather than a delta, so writes
// here are replacements. That is the backend's shape, not a shortcut: a merge
// changes two notes and a take adds a dozen, and a list is the only answer
// that is right for all of them.

import { listNotes } from "./api";
import type { Note } from "./types";

const store = $state({ value: [] as Note[] });

/** The open reading's notes, in reading order. */
export function notes(): Note[] {
  return store.value;
}

/** Replace them — what every note command hands back. */
export function setNotes(next: Note[]) {
  store.value = next;
}

/** Re-read them from the backend. Used after opening a reading, where the
 * notes come from the store rather than from the call that opened it. */
export async function refreshNotes() {
  store.value = await listNotes();
}

/** Leaving a reading leaves its notes with it. */
export function clearNotes() {
  store.value = [];
}
