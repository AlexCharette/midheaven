// Mirrors contract::ChartData — the same JSON the artifact embeds.

export interface ChartData {
  meta: {
    name: string;
    born: string;
    place: string;
    system: string;
    zodiac: string;
    astrologer?: string;
    logo?: string;
  };
  axes: { asc: number; mc: number };
  houseCusps: number[];
  planets: Body[];
  signs: Ref[];
  houses: HouseRef[];
  aspects: Aspect[];
  excerpts: Excerpt[];
}
export interface Body {
  id: string;
  glyph: string;
  name: string;
  lon: number;
  house: number;
}
interface Ref {
  id: string;
  glyph: string;
  name: string;
  element: string;
}
interface HouseRef {
  id: string;
  label: string;
  name: string;
}
interface Aspect {
  id: string;
  glyph: string;
  name: string;
  a: string;
  b: string;
  /** "harmonious" | "challenging" | "neutral" — chord coloring. */
  nature: string;
}
export interface Excerpt {
  id: string;
  time: string;
  span: [number, number];
  text: string;
  tags: string[];
}

export interface PlaceDto {
  id: number;
  label: string;
}

/** Mirrors prefs::Preferences — every field optional; None ⇔ null. */
export interface Preferences {
  models_dir: string | null;
  default_model: string | null;
  readings_dir: string | null;
  astrologer: string | null;
  logo: string | null;
  page_size: string | null;
}

export interface BirthForm {
  name: string;
  date: string;
  time: string;
  place_id: number;
  transcript: string | null;
  model: string | null;
}

/** Force text presentation — glyphs must engrave, not render as emoji. */
export const textGlyph = (g: string) => g + "\ufe0e";

export const catOf = (tag: string) => tag.split(":")[0];

export const norm360 = (x: number) => ((x % 360) + 360) % 360;
export const degInSign = (lon: number) => Math.floor(norm360(lon) % 30);
export const signAt = (chart: ChartData, lon: number) => chart.signs[Math.floor(norm360(lon) / 30)];
export const planetById = (chart: ChartData, id: string) => chart.planets.find((p) => p.id === id);

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
