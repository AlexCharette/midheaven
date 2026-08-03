// What the app is doing right now, and the one place that sets it.
//
// The phase used to be a field on a shared `app` object, so six sites across
// three files wrote it — each with its own `try/finally` to put it back, and one
// of them (the recording path) setting and resetting around an await two
// statements apart. `during` owns that pairing instead: callers say what they
// are doing, not when to stop saying it.

/** Idle, a synchronous chart build, or transcription at a whole-percent
 * progress. One discriminated shape, so consumers ask `isBusy()` and read
 * `.pct` only in the transcribe case — no `typeof`/`!== false` probing. */
export type Busy =
  | { kind: "idle" }
  | { kind: "compute" }
  | { kind: "transcribe"; pct: number };

const IDLE: Busy = { kind: "idle" };

const store = $state({ value: IDLE as Busy });

/** What the app is doing. */
export const busy = (): Busy => store.value;

/** Whether any operation is in flight (a build or a transcription). */
export const isBusy = (): boolean => store.value.kind !== "idle";

/** Run `work` with the phase set, and put it back however it ends — including
 * on the error path, which is what the hand-written `finally` blocks were for.
 * Returns whatever `work` returns, so call sites read as ordinary awaits. */
export async function during<T>(kind: Busy["kind"], work: () => Promise<T>): Promise<T> {
  store.value = kind === "transcribe" ? { kind: "transcribe", pct: 0 } : { kind };
  try {
    return await work();
  } finally {
    store.value = IDLE;
  }
}

/** Report transcription progress. Ignored unless a transcription is the current
 * phase, so a late event from a finished take cannot re-busy an idle app. */
export function setProgress(pct: number) {
  if (store.value.kind === "transcribe") {
    store.value = { kind: "transcribe", pct };
  }
}
