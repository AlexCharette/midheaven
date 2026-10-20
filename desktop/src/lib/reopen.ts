// Reopening a stored chart as the current reading — one path for every place
// that lists charts. It used to live inside the library panel, which was the
// only list; the calculator now shows the most recent ones as well, and two
// copies of "compute, install, read back the notes" would drift.

import { openChart } from "./api";
import { during, isBusy } from "./busy.svelte";
import { refreshNotes } from "./notes.svelte";
import { openReading, whyCannotOpenAnother } from "./session.svelte";
import { notify } from "./toasts.svelte";

/** Open chart `id` as the reading. Refused (a no-op) while busy or mid-take —
 * it would re-seat the chart under a live recorder, and the take would land on
 * this reading instead of the one it was spoken over. Throws what the backend
 * refused, for the caller to show where the click was. */
export async function reopenReading(id: string): Promise<void> {
  if (isBusy() || whyCannotOpenAnother()) return;
  const chart = await during("compute", () => openChart(id));
  if (openReading(chart)) {
    await refreshNotes();
    notify(`opened ${chart.meta.name}'s reading`);
  }
}
