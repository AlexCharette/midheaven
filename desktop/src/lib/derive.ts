// The client mirror of `src/derive.rs`, and the only longitude arithmetic left
// in the webview.
//
// Derived values normally arrive ON the contract — a `Body` carries `sign`,
// `deg` and `min`, and a `ChartData` carries `houseSweeps`, all computed once in
// Rust. Read those fields wherever a chart is being displayed as it was
// computed; do not re-derive them here.
//
// Two cases genuinely cannot read the fields, and are why this file exists:
//
//   1. The live scrub. `Calculator` renders a pose-tweened chart whose
//      longitudes are interpolated between previews, so the fields the backend
//      filled belong to the target, not to the frame on screen. The splice
//      refills them through `positionOf`/`sweepOf` as it moves the longitudes.
//   2. House cusps. A cusp is a longitude the chart carries, not a body, so it
//      has no derived position of its own to read.
//
// Because it is a mirror, `derive.test.ts` pins it against the same cases as
// `src/derive.rs`'s own tests — including the minute-carry that rolls into the
// next sign, which the copies this file replaced all got wrong.

/** Normalize any degree value into [0, 360) — the mirror of `Longitude::new`. */
export const norm360 = (x: number) => ((x % 360) + 360) % 360;

/** Where a longitude falls: which sign (0..=11), and how far into it. */
export type Position = { sign: number; deg: number; min: number };

/** The mirror of `derive::position`.
 *
 * The minute is *rounded*, so it can carry: at 29°59.7′ the minutes round to
 * 60, which advances the degree to 30 — and a 30th degree does not exist, so
 * the carry rolls on into the next sign. */
export function positionOf(lon: number): Position {
  const d = norm360(lon);
  let sign = Math.floor(d / 30);
  const within = d % 30;
  let deg = Math.floor(within);
  let min = Math.round((within - deg) * 60);
  if (min >= 60) {
    min = 0;
    deg += 1;
  }
  if (deg >= 30) {
    deg = 0;
    sign += 1;
  }
  return { sign: sign % 12, deg, min };
}

/** The mirror of `derive::sweep`: how wide a house is, cusp to next cusp.
 *
 * Coincident cusps mean a full sign — quadrant systems can collapse two cusps
 * onto each other at extreme latitudes, and a zero-width wedge would vanish
 * from the wheel instead of spanning its sign. */
export function sweepOf(cusp: number, next: number): number {
  const arc = norm360(next - cusp);
  return arc === 0 ? 30 : arc;
}
