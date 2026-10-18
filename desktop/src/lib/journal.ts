// What a journal note is about, and how to find one again.
//
// Rune-free and pure, so it is directly testable; the components hold the
// state. The same split as `focus.ts`/`focus.svelte.ts`.

import type { Chart, ChartData, JournalRow, PreviewInput } from "./types";

/**
 * The anchor for a note written over a previewed sky.
 *
 * Almost all of it comes from the chart rather than from the request, and that
 * is the point: the calculator's codes may be *blank*, which means "not
 * chosen" and is answered by the backend's own default ladder. The chart
 * carries what those blanks resolved to. Recording the request's codes would
 * store "unstated" and let the note drift the day a preference changes.
 *
 * The place id is the exception — a preview does not stamp `meta.birth`, so the
 * only account of which gazetteer entry this was is the request's.
 */
export function anchorFor(
  input: PreviewInput,
  chart: ChartData,
  person?: string | null,
): Chart {
  return {
    // Minted backend-side on the first note about this sky, so every note
    // over one moment shares one chart. Sent empty so the type is whole.
    id: "",
    kind: "moment",
    seed: { place_id: input.place_id, date: input.date, time: input.time },
    // Overwritten backend-side from the resolved place; sent so the type is
    // whole, and so a row still reads correctly if that ever stops happening.
    place: chart.meta.place,
    houseSystem: chart.meta.house_system,
    ...(chart.meta.ayanamsa ? { ayanamsa: chart.meta.ayanamsa } : {}),
    locale: chart.meta.locale,
    ...(person ? { person } : {}),
  };
}

/** Who a row is about, for display: the person when there is one, else the
 * place the sky was over.
 *
 * A standing note is about neither — it is about its elements, and the row
 * already renders those as chips. Repeating them here would be saying the
 * same thing twice, so it says nothing and the mark carries the distinction.
 */
export function rowSubject(row: JournalRow): string {
  return row.person?.name ?? row.chart?.place ?? "";
}

/** The engraved mark that tells the three kinds apart at a glance.
 *
 * A mark rather than a colour: the palette is CVD-validated and PRODUCT.md
 * is explicit that colour is never the only signal. */
export function rowMark(row: JournalRow): { glyph: string; title: string } {
  switch (row.anchor.kind) {
    case "chart":
      return { glyph: "☍", title: "said in a reading" };
    case "person":
      return { glyph: "☉", title: "about this person" };
    case "standing":
      return { glyph: "✶", title: "a standing note" };
  }
}

/**
 * Whether a row matches a search.
 *
 * Three sources, one field: the words, who it is about, and the elements it is
 * filed under — because "mira", "saturn" and a half-remembered phrase are the
 * three ways anyone reaches for a note, and making the person pick which kind
 * of thing they are typing would be making them do the work.
 *
 * Elements match on `tagLabels`, not on `tags`: nobody searches for
 * `planet:saturn`, and a note written under a Russian reading carries Russian
 * labels so it answers to Russian.
 */
export function matches(row: JournalRow, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [row.text, rowSubject(row), ...row.tagLabels].some((h) =>
    h.toLowerCase().includes(q),
  );
}

/** The rows a search leaves. Order is the backend's and is never re-sorted
 * here. */
export function search(rows: JournalRow[], query: string): JournalRow[] {
  return rows.filter((r) => matches(r, query));
}
