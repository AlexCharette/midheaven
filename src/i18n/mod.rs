//! Per-locale display names and router match-terms, keyed by the
//! language-neutral catalog (`chart::catalog`). This is the one place a
//! language's astrology vocabulary lives; adding a language means adding a
//! table module (`en`, `ru`, …) and one arm in [`Locale::parse`]/[`Locale::table`].
//!
//! The catalog stays language-neutral (slugs, glyphs, elements, ids, angles);
//! only the *text* — element names and the words a reader would say — is here.
//! Tag-ids (`planet:sun`) never change across locales, so a reading's language
//! is a presentation/routing concern, not a contract change.

mod en;
mod ru;

/// A supported reading language. Stored on `ChartData.meta.locale` as a short
/// code so it round-trips through the artifact and reloaded `chart.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En,
    Ru,
}

impl Locale {
    /// Parse a stored/selected locale code; unknown or empty → the default
    /// (`En`), so a chart with no `locale` (older files) reads as English.
    pub fn parse(code: &str) -> Locale {
        match code.trim().to_lowercase().as_str() {
            "ru" | "rus" | "russian" | "русский" => Locale::Ru,
            _ => Locale::En,
        }
    }

    /// The short code persisted in `meta.locale`.
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
        }
    }

    /// The whisper language hint for this locale (stage 1). Telling whisper the
    /// language beats auto-detect on a known reading language.
    pub fn whisper_lang(self) -> &'static str {
        self.code()
    }

    /// Every locale the app can produce a reading in — for building selectors.
    pub const ALL: &'static [Locale] = &[Locale::En, Locale::Ru];

    /// The language's own name (endonym), shown in UI language selectors.
    pub fn endonym(self) -> &'static str {
        self.table().endonym
    }

    /// The word every house name ends with (`" House"`, `" дом"`); a viewer
    /// strips it to show the bare ordinal ("First") — the one home for that
    /// mapping, so no frontend re-encodes it.
    pub fn house_suffix(self) -> &'static str {
        self.table().house_suffix
    }

    fn table(self) -> &'static LocaleTable {
        match self {
            Locale::En => &en::TABLE,
            Locale::Ru => &ru::TABLE,
        }
    }

    // ---- display names (feed ChartData; flow to every renderer) ----

    /// Display name for a planet slug (`sun`, …, `ascendant`).
    pub fn planet_name(self, slug: &str) -> &'static str {
        entry(self.table().planets, slug)
            .or_else(|| entry(en::TABLE.planets, slug))
            .map_or("", |e| e.name)
    }

    /// Display name for a sign slug (`aries` … `pisces`).
    pub fn sign_name(self, slug: &str) -> &'static str {
        entry(self.table().signs, slug)
            .or_else(|| entry(en::TABLE.signs, slug))
            .map_or("", |e| e.name)
    }

    /// Full display name for a 1-based house number ("First House" / "Первый дом").
    pub fn house_name(self, n: usize) -> &'static str {
        self.house(n).map_or("", |h| h.name)
    }

    /// Compose an aspect's display name, e.g. "Sun trine Moon" / "Солнце тригон Луна".
    pub fn aspect_name(self, kind: &str, a_name: &str, b_name: &str) -> String {
        let word = self.aspect_word(kind);
        format!("{a_name} {word} {b_name}")
    }

    /// The bare aspect word used in the name ("trine" / "тригон"). Aspect kinds
    /// come from the fixed catalog and English lists them all, so the empty
    /// fallback is unreachable in practice.
    pub fn aspect_word(self, kind: &str) -> &'static str {
        aspect(self.table().aspects, kind)
            .or_else(|| aspect(en::TABLE.aspects, kind))
            .map_or("", |a| a.word)
    }

    /// Default house-system label ("Whole Sign" / "Целые знаки"). Kept for the
    /// Whole-Sign default; per-system labels come from [`house_system_label`].
    pub fn system_label(self) -> &'static str {
        self.table().system
    }

    /// Display label for a house-system code (`placidus` → "Placidus"), falling
    /// back to the English table, then to the default label, so an unlisted
    /// code still renders something sensible rather than blank.
    pub fn house_system_label(self, code: &str) -> &'static str {
        lookup_label(self.table().house_systems, code)
            .or_else(|| lookup_label(en::TABLE.house_systems, code))
            .unwrap_or(self.table().system)
    }

    /// Default zodiac label ("Tropical" / "Тропический").
    pub fn zodiac_label(self) -> &'static str {
        self.table().zodiac
    }

    /// Zodiac label for `meta.zodiac`: the tropical word, or the sidereal word
    /// joined to the ayanamsa's own name (a proper noun, so not translated).
    pub fn zodiac_label_for(self, ayanamsa: Option<xalen_ayanamsa::Ayanamsa>) -> String {
        match ayanamsa {
            None => self.zodiac_label().to_string(),
            Some(a) => format!("{} · {a}", self.table().sidereal),
        }
    }

    /// The persona used when no name is given.
    pub fn anonymous(self) -> &'static str {
        self.table().anonymous
    }

    /// Fixed PDF chrome for this locale.
    /// The emitted artifact's chrome. No English fallback: a locale with a
    /// table has a complete one, and `parse` already sent anything unknown to
    /// English before it got here.
    pub fn artifact(self) -> &'static ArtifactChrome {
        &self.table().artifact
    }

    /// How to title a chart's plate in this language.
    pub fn plate_title(self) -> PlateTitle {
        self.table().plate_title
    }

    pub fn pdf(self) -> &'static PdfChrome {
        &self.table().pdf
    }

    /// The plate's figure caption, composed with the locale's grammar. `system`
    /// and `zodiac` are already-localized `meta` strings.
    pub fn pdf_figure_caption(
        self,
        name: &str,
        born: &str,
        place: &str,
        system: &str,
        zodiac: &str,
    ) -> String {
        let place = if place.is_empty() { String::new() } else { format!(", {place}") };
        match self {
            Locale::En => format!(
                "Fig. I. \u{2014} The natal figure of {name}, calculated for {born}{place}. \
                 {system} houses upon the {} zodiac.",
                zodiac.to_lowercase()
            ),
            Locale::Ru => format!(
                "Рис. I. \u{2014} Натальная карта {name}, рассчитана на {born}{place}. \
                 Система домов: {system}. Зодиак: {zodiac}."
            ),
        }
    }

    // ---- router match-terms (stage 3) ----

    /// Lowercase terms a reader would use for a planet slug.
    pub fn planet_terms(self, slug: &str) -> &'static [&'static str] {
        entry(self.table().planets, slug).map_or(&[], |e| e.terms)
    }

    /// Lowercase terms a reader would use for a sign slug.
    pub fn sign_terms(self, slug: &str) -> &'static [&'static str] {
        entry(self.table().signs, slug).map_or(&[], |e| e.terms)
    }

    /// Lowercase terms a reader would use for a 1-based house number.
    pub fn house_terms(self, n: usize) -> &'static [&'static str] {
        self.house(n).map_or(&[], |h| h.terms)
    }

    /// Lowercase words that name an aspect kind in speech.
    pub fn aspect_match_words(self, kind: &str) -> &'static [&'static str] {
        aspect(self.table().aspects, kind).map_or(&[], |a| a.match_words)
    }

    fn house(self, n: usize) -> Option<&'static HouseEntry> {
        n.checked_sub(1)
            .and_then(|i| self.table().houses.get(i))
            .or_else(|| n.checked_sub(1).and_then(|i| en::TABLE.houses.get(i)))
    }
}

/// A locale's complete vocabulary. Tables are `static` data in `en`/`ru`.
pub struct LocaleTable {
    /// Keyed by planet slug — includes `ascendant`, which the catalog adds
    /// as a point rather than a body.
    pub planets: &'static [Entry],
    /// Keyed by sign slug, zodiac order.
    pub signs: &'static [Entry],
    /// House 1 first (index 0).
    pub houses: &'static [HouseEntry],
    /// Keyed by aspect kind (`conjunction`, `sextile`, …).
    pub aspects: &'static [AspectEntry],
    pub system: &'static str,
    pub zodiac: &'static str,
    /// House-system display labels keyed by wire code (`chart::systems`).
    pub house_systems: &'static [(&'static str, &'static str)],
    /// The word for the sidereal zodiac ("Sidereal" / "Сидерический"), joined
    /// to the ayanamsa's own name in [`Locale::zodiac_label_for`].
    pub sidereal: &'static str,
    pub anonymous: &'static str,
    /// The language's own name (endonym), for UI language selectors.
    pub endonym: &'static str,
    /// The trailing word shared by every house name (`" House"`, `" дом"`) —
    /// lets a viewer show the bare ordinal without re-encoding the mapping.
    pub house_suffix: &'static str,
    /// How this language titles a chart's plate — shared by both renditions.
    pub plate_title: PlateTitle,
    pub pdf: PdfChrome,
    pub artifact: ArtifactChrome,
}

/// How a language titles a chart's plate. The *shape* differs, not only the
/// words, which is why this is a choice and not a string.
///
/// English takes a possessive, so the holder's name is inside the phrase and the
/// plate has one line. Russian cannot: a possessive there requires declining the
/// name into the genitive, which no format string can do, so it keeps the line
/// above the name and lets the name stand undeclined beneath it.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlateTitle {
    /// One line, with `{name}` substituted into it. No separate name follows.
    Inline(&'static str),
    /// A line set above the name, which the rendition sets separately.
    Above(&'static str),
}

impl PlateTitle {
    /// The plate's headline for a holder, and the name to set beneath it —
    /// `None` when the headline already contains it.
    pub fn render(self, name: &str) -> (String, Option<&str>) {
        match self {
            PlateTitle::Inline(fmt) => (fmt.replace("{name}", name), None),
            PlateTitle::Above(line) => (line.to_string(), Some(name)),
        }
    }
}

/// A named element and the words that route to it.
pub struct Entry {
    pub slug: &'static str,
    pub name: &'static str,
    pub terms: &'static [&'static str],
}

pub struct HouseEntry {
    pub name: &'static str,
    pub terms: &'static [&'static str],
}

pub struct AspectEntry {
    pub kind: &'static str,
    /// Word used to compose the aspect's display name.
    pub word: &'static str,
    /// Words that fire the aspect when both planets co-occur.
    pub match_words: &'static [&'static str],
}

/// Fixed rubrics rendered into the PDF. The artifact's own furniture is
/// [`ArtifactChrome`]; the two are separate because the renditions differ (the
/// artifact has filter controls and empty states, paper has neither), but both
/// live here.
pub struct PdfChrome {
    /// Branding line before the astrologer's name (rendered uppercase).
    pub prepared_by: &'static str,
    pub index_of_elements: &'static str,
    pub commentary: &'static str,
}

/// Everything on the emitted artifact's page that is not chart data.
///
/// It used to be a `UI = { en, ru }` object inside `templates/reading.html`,
/// parallel to this module rather than derived from it: adding a language meant
/// editing a Rust table *and* an HTML template, and the two had already drifted
/// (see [`ArtifactChrome::birth_chart_of`]). `emit` now substitutes this beside
/// the chart, so the template holds no strings of its own.
///
/// The three fields ending in a placeholder are format strings; the viewer
/// substitutes `{…}` by name.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactChrome {
    /// Browser tab title, after the holder's name.
    pub natal_reading: &'static str,
    pub index_of_elements: &'static str,
    /// Column heads of the index, in order: planets, signs, houses, aspects.
    pub bands: [&'static str; 4],
    pub passages_touching: &'static str,
    pub any: &'static str,
    pub all: &'static str,
    pub any_title: &'static str,
    pub all_title: &'static str,
    pub of_selection: &'static str,
    pub clear: &'static str,
    pub commentary: &'static str,
    pub prepared_by: &'static str,
    pub wheel_aria: &'static str,
    /// `{shown}` of `{total}` passages.
    pub count: &'static str,
    pub no_passages_routed: &'static str,
    pub empty_none_routed: &'static str,
    /// `{word}` is [`any`](Self::any) or [`all`](Self::all).
    pub empty_no_match: &'static str,
    pub fewer: &'static str,
    /// `{n}` more.
    pub more: &'static str,
}

fn entry(table: &'static [Entry], slug: &str) -> Option<&'static Entry> {
    table.iter().find(|e| e.slug == slug)
}

fn lookup_label(table: &'static [(&'static str, &'static str)], code: &str) -> Option<&'static str> {
    table.iter().find(|(c, _)| *c == code).map(|(_, label)| *label)
}

fn aspect(table: &'static [AspectEntry], kind: &str) -> Option<&'static AspectEntry> {
    table.iter().find(|a| a.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every locale must ship complete artifact chrome. This was a `UI` object
    /// in an HTML template with no check at all — a language could be added to
    /// Rust and the artifact would silently render English furniture around its
    /// data. Now the table will not compile without it, and this pins the
    /// strings against being left blank.
    #[test]
    fn every_locale_ships_complete_artifact_chrome() {
        for loc in Locale::ALL {
            let a = loc.artifact();
            let singles = [
                ("natalReading", a.natal_reading),
                ("indexOfElements", a.index_of_elements),
                ("passagesTouching", a.passages_touching),
                ("any", a.any),
                ("all", a.all),
                ("anyTitle", a.any_title),
                ("allTitle", a.all_title),
                ("ofSelection", a.of_selection),
                ("clear", a.clear),
                ("commentary", a.commentary),
                ("preparedBy", a.prepared_by),
                ("wheelAria", a.wheel_aria),
                ("count", a.count),
                ("noPassagesRouted", a.no_passages_routed),
                ("emptyNoneRouted", a.empty_none_routed),
                ("emptyNoMatch", a.empty_no_match),
                ("fewer", a.fewer),
                ("more", a.more),
            ];
            for (field, value) in singles {
                assert!(!value.trim().is_empty(), "{:?} has an empty {field}", loc.code());
            }
            assert!(
                a.bands.iter().all(|b| !b.trim().is_empty()),
                "{:?} has a blank index column head",
                loc.code()
            );
            let (headline, _) = loc.plate_title().render("Mira Holt");
            assert!(!headline.trim().is_empty(), "{:?} has a blank plate title", loc.code());
        }
    }

    /// The two shapes a plate title takes. English's possessive puts the
    /// holder's name inside the headline; Russian's cannot, because the name
    /// would have to be declined into the genitive, so it sets the name
    /// separately beneath an unchanging line.
    #[test]
    fn a_plate_title_either_carries_the_name_or_hands_it_back() {
        for loc in Locale::ALL {
            let (headline, beneath) = loc.plate_title().render("Mira Holt");
            match beneath {
                None => assert!(
                    headline.contains("Mira Holt"),
                    "{:?} sets no name beneath, so the headline must carry it: {headline:?}",
                    loc.code()
                ),
                Some(name) => {
                    assert_eq!(name, "Mira Holt");
                    assert!(
                        !headline.contains("Mira Holt"),
                        "{:?} sets the name beneath, so the headline must not repeat it",
                        loc.code()
                    );
                }
            }
            assert!(!headline.contains('{'), "{:?} left a placeholder unfilled", loc.code());
        }
    }

    #[test]
    fn english_titles_a_plate_with_a_possessive() {
        let (headline, beneath) = Locale::En.plate_title().render("Mira Holt");
        assert_eq!(headline, "Mira Holt's birth chart");
        assert_eq!(beneath, None, "one line, so nothing is set beneath it");
    }

    #[test]
    fn russian_keeps_the_name_undeclined_beneath_its_title() {
        let (headline, beneath) = Locale::Ru.plate_title().render("Мира Холт");
        assert_eq!(headline, "Натальная карта");
        assert_eq!(beneath, Some("Мира Холт"));
    }

    /// The chrome's format strings carry the placeholders the viewer
    /// substitutes. A typo here renders a literal `{shown}` on the page.
    #[test]
    fn the_chrome_format_strings_carry_their_placeholders() {
        for loc in Locale::ALL {
            let a = loc.artifact();
            let code = loc.code();
            assert!(a.count.contains("{shown}") && a.count.contains("{total}"), "{code}: count");
            assert!(a.empty_no_match.contains("{word}"), "{code}: emptyNoMatch");
            assert!(a.more.contains("{n}"), "{code}: more");
            // And no other field pretends to take one.
            for (field, value) in [
                ("natalReading", a.natal_reading),
                ("clear", a.clear),
                ("fewer", a.fewer),
            ] {
                assert!(!value.contains('{'), "{code}: {field} has a stray placeholder");
            }
        }
    }

    #[test]
    fn parse_is_lenient_and_defaults_to_english() {
        assert_eq!(Locale::parse("ru"), Locale::Ru);
        assert_eq!(Locale::parse("RU"), Locale::Ru);
        assert_eq!(Locale::parse("русский"), Locale::Ru);
        assert_eq!(Locale::parse(""), Locale::En);
        assert_eq!(Locale::parse("fr"), Locale::En);
        assert_eq!(Locale::default(), Locale::En);
    }

    #[test]
    fn english_names_match_the_legacy_catalog_strings() {
        // These must stay byte-identical so English output never shifts.
        let en = Locale::En;
        assert_eq!(en.planet_name("sun"), "Sun");
        assert_eq!(en.planet_name("ascendant"), "Ascendant");
        assert_eq!(en.sign_name("leo"), "Leo");
        assert_eq!(en.house_name(5), "Fifth House");
        assert_eq!(en.system_label(), "Whole Sign");
        assert_eq!(en.zodiac_label(), "Tropical");
        assert_eq!(en.house_system_label("placidus"), "Placidus");
        assert_eq!(en.house_system_label("whole-sign"), "Whole Sign");
        assert_eq!(en.zodiac_label_for(None), "Tropical");
        assert!(en.zodiac_label_for(Some(xalen_ayanamsa::Ayanamsa::Lahiri)).starts_with("Sidereal"));
        assert_eq!(en.aspect_name("trine", "Sun", "Moon"), "Sun trine Moon");
        assert_eq!(en.anonymous(), "Anonymous");
    }

    #[test]
    fn russian_has_names_and_terms_for_every_element() {
        let ru = Locale::Ru;
        assert_eq!(ru.planet_name("sun"), "Солнце");
        assert_eq!(ru.sign_name("cancer"), "Рак");
        assert!(ru.house_name(5).contains("дом"));
        assert_eq!(ru.house_system_label("whole-sign"), "Целые знаки");
        assert_eq!(ru.house_system_label("placidus"), "Плацидус");
        assert!(ru.planet_terms("sun").contains(&"солнце"));
        assert!(ru.sign_terms("cancer").contains(&"рак"));
        assert!(ru.house_terms(5).iter().any(|t| t.contains("дом")));
        assert!(!ru.aspect_match_words("trine").is_empty());
    }
}
