// Transient status notifications — each auto-dismisses (errors linger longer)
// and any can be dismissed by a click. Replaced the old persistent footer
// status line.

export type Toast = { id: number; message: string; kind: "info" | "error" };

export const toasts = $state<Toast[]>([]);
let nextToastId = 0;

/** How long a toast stays up. Errors linger, because they are read rather than
 * glanced at. */
const LINGER_MS = { info: 4000, error: 7000 } as const;

export function notify(message: string, kind: "info" | "error" = "info") {
  const id = nextToastId++;
  toasts.push({ id, message, kind });
  setTimeout(() => dismissToast(id), LINGER_MS[kind]);
}

export function dismissToast(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i !== -1) toasts.splice(i, 1);
}
