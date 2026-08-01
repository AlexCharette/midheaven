// The contract types (ChartData & co.) and the backend DTOs are GENERATED
// from the Rust structs by ts-rs — see `./generated/`. Do not hand-edit those
// files or re-declare the shapes here; regenerate with `npm run gen:types` so
// a Rust rename can never silently drift from the webview. This module
// re-exports them under the familiar `$lib/types` path and adds the pure
// client-side derivations.

export type { ChartData } from "./generated/ChartData";
export type { Meta } from "./generated/Meta";
export type { BirthSeed } from "./generated/BirthSeed";
export type { Axes } from "./generated/Axes";
export type { Body } from "./generated/Body";
export type { Ref } from "./generated/Ref";
export type { HouseRef } from "./generated/HouseRef";
export type { Aspect } from "./generated/Aspect";
export type { Excerpt } from "./generated/Excerpt";
export type { PlaceDto } from "./generated/PlaceDto";
export type { Preferences } from "./generated/Preferences";
export type { ReadingEntry } from "./generated/ReadingEntry";
export type { AppChrome } from "./generated/AppChrome";
export type { BirthForm } from "./generated/BirthForm";
export type { CalculationDefaults } from "./generated/CalculationDefaults";
export type { LocaleDto } from "./generated/LocaleDto";
export type { OptionDto } from "./generated/OptionDto";
export type { PreviewInput } from "./generated/PreviewInput";
export type { PreviewDto } from "./generated/PreviewDto";

import type { Body } from "./generated/Body";
import type { ChartData } from "./generated/ChartData";

/** Force text presentation — glyphs must engrave, not render as emoji. */
export const textGlyph = (g: string) => g + "\ufe0e";

export const catOf = (tag: string) => tag.split(":")[0];

export const planetById = (chart: ChartData, id: string) => chart.planets.find((p) => p.id === id);

/** The sign a body stands in, read from its derived `sign` index rather than
 * re-derived from `lon` — see `$lib/derive` for why the webview no longer owns
 * that arithmetic. */
export const signOf = (chart: ChartData, body: Body) => chart.signs[body.sign];

/** The two zodiac wire codes.
 *
 * House systems and ayanamsas are served by the backend so the webview never
 * re-encodes them; the zodiac's two have no list command, so this is their one
 * client home. They are vocabulary, not defaults — `tropical` happens to be the
 * default zodiac, but it is the code for a non-sidereal chart whatever the
 * default becomes. */
export const TROPICAL = "tropical";
export const SIDEREAL = "sidereal";

/** The calculation a chart was computed with, read from its own metadata.
 *
 * `meta` carries the house-system and ayanamsa *codes* but only a display label
 * for the zodiac, so the zodiac code comes from the ayanamsa: the contract sets
 * one exactly when the chart is sidereal. That inference used to live inside an
 * `$effect` in the reading view and was its only statement anywhere.
 *
 * `fallback` covers a chart saved before `meta.house_system` existed. */
export function calculationOf(
  chart: ChartData,
  fallback: { houseSystem: string; ayanamsa: string },
): { houseSystem: string; zodiac: string; ayanamsa: string } {
  return {
    houseSystem: chart.meta.house_system || fallback.houseSystem,
    zodiac: chart.meta.ayanamsa ? SIDEREAL : TROPICAL,
    ayanamsa: chart.meta.ayanamsa ?? fallback.ayanamsa,
  };
}

/** Every taggable element as {tag, glyph, name}, encoding the one
 * per-category glyph convention (houses show their roman label). */
export function elementsOf(chart: ChartData): { tag: string; glyph: string; name: string }[] {
  return [
    ...chart.planets.map((x) => ({ tag: x.id, glyph: x.glyph, name: x.name })),
    ...chart.signs.map((x) => ({ tag: x.id, glyph: x.glyph, name: x.name })),
    ...chart.houses.map((x) => ({ tag: x.id, glyph: x.label, name: x.name })),
    ...chart.aspects.map((x) => ({ tag: x.id, glyph: x.glyph, name: x.name })),
  ];
}

// What lights up when an element is focused — `relatedTo`, `signDensity`,
// occupancy — lives in `$lib/focus`, beside the focus rules that consume it.
