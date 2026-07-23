// Mirrors contract::ChartData — the same JSON the artifact embeds.

export interface ChartData {
  meta: {
    name: string;
    born: string;
    place: string;
    system: string;
    zodiac: string;
    /** Reading language code ("en", "ru"); may be absent on older charts. */
    locale?: string;
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
  default_locale: string | null;
}

/** Mirrors the backend's ReadingEntry — one row of the saved-readings library. */
export interface ReadingEntry {
  chartPath: string;
  dir: string;
  name: string;
  born: string;
  place: string;
  excerpts: number;
  modifiedMs: number | null;
}

export interface BirthForm {
  name: string;
  date: string;
  time: string;
  place_id: number;
  transcript: string | null;
  model: string | null;
  /** Reading language code ("en", "ru"); backend falls back to the
   * default-language preference, then English. */
  lang: string | null;
}

/** Reading languages offered in the UI — mirrors i18n::Locale::ALL.
 * `label` is shown in the language's own script (endonym). */
export const LOCALES: { code: string; label: string }[] = [
  { code: "en", label: "English" },
  { code: "ru", label: "Русский" },
];

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

/** The elements a given element is bound to, so the orrery can light up every
 * relation at once when one is focused. The set never includes the focus tag
 * itself — that's styled as `.focus`, its relations as `.rel`. Symmetric:
 * focusing a planet lights its sign/house/aspects/partners; focusing any of
 * those lights the planet back.
 *   planet  → its sign, its house, its aspects, the far planet of each aspect
 *   sign    → the planets standing in it
 *   house   → the planets tenanting it
 *   aspect  → its two endpoint planets */
export function relatedTo(chart: ChartData, tag: string): Set<string> {
  const rel = new Set<string>();
  const cat = catOf(tag);
  if (cat === "planet") {
    const p = planetById(chart, tag);
    if (p) {
      rel.add(signAt(chart, p.lon).id);
      rel.add(`house:${p.house}`);
      for (const a of chart.aspects) {
        if (a.a === tag) { rel.add(a.id); rel.add(a.b); }
        else if (a.b === tag) { rel.add(a.id); rel.add(a.a); }
      }
    }
  } else if (cat === "sign") {
    for (const p of chart.planets) if (signAt(chart, p.lon).id === tag) rel.add(p.id);
  } else if (cat === "house") {
    const n = Number(tag.split(":")[1]);
    for (const p of chart.planets) if (p.house === n) rel.add(p.id);
  } else if (cat === "aspect") {
    const a = chart.aspects.find((x) => x.id === tag);
    if (a) { rel.add(a.a); rel.add(a.b); }
  }
  return rel;
}

/** Passage weight per sign (index 0 = Aries … 11 = Pisces), for the outer
 * density track: how much the reading dwelt on each sign. A passage counts
 * toward a sign when it is tagged with that sign, or with a planet standing in
 * it — so a talkative Sun lights up its own sign even when the words never
 * named the sign directly. */
export function signDensity(chart: ChartData): number[] {
  const planetSign = new Map(
    chart.planets.map((p) => [p.id, Math.floor(norm360(p.lon) / 30)]),
  );
  const weight = new Array(12).fill(0);
  for (const ex of chart.excerpts) {
    const hit = new Set<number>();
    for (const tag of ex.tags) {
      if (tag.startsWith("sign:")) {
        const i = chart.signs.findIndex((s) => s.id === tag);
        if (i !== -1) hit.add(i);
      } else if (planetSign.has(tag)) {
        hit.add(planetSign.get(tag)!);
      }
    }
    for (const i of hit) weight[i]++;
  }
  return weight;
}
