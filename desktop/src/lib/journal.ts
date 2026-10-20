// What a journal note is about, and how to find one again.
//
// Rune-free and pure, so it is directly testable; the components hold the
// state. The same split as `focus.ts`/`focus.svelte.ts`.

import type { Chart, ChartData, ChartRow, JournalRow, PreviewInput } from "./types";

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

/**
 * The chart the journal draws for a person: their natal chart if the store
 * has one, else the first chart of theirs it holds. `null` when there is
 * none — the journal then keeps the vocabulary wheel.
 */
export function natalOf(person: string, charts: ChartRow[]): Chart | null {
  const theirs = charts.map((r) => r.chart).filter((c) => c.person === person);
  return theirs.find((c) => c.kind === "natal") ?? theirs[0] ?? null;
}

/** A stored chart as a preview request — the side-effect-free way to draw
 * it. `open_chart` would install it as the open reading, which the journal
 * has no business doing just to show a wheel. */
export function previewOf(chart: Chart): PreviewInput {
  return {
    date: chart.seed.date,
    time: chart.seed.time,
    place_id: chart.seed.place_id,
    lang: chart.locale,
    house_system: chart.houseSystem,
    zodiac: chart.ayanamsa ? "sidereal" : "tropical",
    ayanamsa: chart.ayanamsa ?? null,
  };
}

type Pt = { x: number; y: number };

/**
 * How the journal's stage zooms onto an element: the transform that scales
 * the plate by `scale` about its top-left and carries the element at `at`
 * (stage coordinates, untransformed) to a resting point, plus which side of
 * that point the floating notes go.
 *
 * The notes go *outward* — toward the side of the plate the element faces —
 * because past the rim there is only sky, and a card laid over sky hides
 * nothing of the chart. An element at the hub faces nowhere; its notes go
 * right. The resting point sits off-centre toward the plate, leaving the
 * outward side room for the cards.
 */
export function zoomOnto(
  at: Pt,
  centre: Pt,
  stage: { w: number; h: number },
  scale: number,
  hubRadius: number,
): { tx: number; ty: number; rest: Pt; side: "left" | "right" } {
  const dx = at.x - centre.x;
  const atHub = Math.hypot(dx, at.y - centre.y) < hubRadius;
  const side: "left" | "right" = atHub || dx >= 0 ? "right" : "left";
  const rest = { x: stage.w * (side === "right" ? 0.36 : 0.64), y: stage.h / 2 };
  return { tx: rest.x - at.x * scale, ty: rest.y - at.y * scale, rest, side };
}
