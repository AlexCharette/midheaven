// One rule for turning something thrown into something to show a person.
//
// Its own module, and not part of `api.ts`, because the rune-free modules use it
// too and must not pull in the Tauri bindings to do so.
//
// The backend writes its refusals for the astrologer ("no chart has been built
// yet", "pick a place from the suggestions"), so the message is the whole
// message — no prefix, no class name. Fourteen call sites each decided this for
// themselves, five with `String(e)` and nine with a template literal, which
// differ the moment anything throws an `Error` rather than a string.

export const reason = (e: unknown): string => (e instanceof Error ? e.message : String(e));
