// Mirrors contract::ChartData — the same JSON the artifact embeds.

export interface ChartData {
  meta: { name: string; born: string; place: string; system: string; zodiac: string };
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
export interface Ref {
  id: string;
  glyph: string;
  name: string;
}
export interface HouseRef {
  id: string;
  label: string;
  name: string;
}
export interface Aspect {
  id: string;
  glyph: string;
  name: string;
  a: string;
  b: string;
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
  tz: string;
  lat: number;
  lon: number;
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
