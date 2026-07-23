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

    /// House-system label for `meta.system` ("Whole Sign" / "Целые знаки").
    pub fn system_label(self) -> &'static str {
        self.table().system
    }

    /// Zodiac label for `meta.zodiac` ("Tropical" / "Тропический").
    pub fn zodiac_label(self) -> &'static str {
        self.table().zodiac
    }

    /// The persona used when no name is given.
    pub fn anonymous(self) -> &'static str {
        self.table().anonymous
    }

    /// Fixed PDF chrome for this locale.
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
    pub anonymous: &'static str,
    /// The language's own name (endonym), for UI language selectors.
    pub endonym: &'static str,
    /// The trailing word shared by every house name (`" House"`, `" дом"`) —
    /// lets a viewer show the bare ordinal without re-encoding the mapping.
    pub house_suffix: &'static str,
    pub pdf: PdfChrome,
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

/// Fixed rubrics rendered into the PDF (the artifact's chrome lives in the
/// template instead — see `templates/reading.html`).
pub struct PdfChrome {
    /// Title line before the holder's name.
    pub nativity_of: &'static str,
    /// Branding line before the astrologer's name (rendered uppercase).
    pub prepared_by: &'static str,
    pub index_of_elements: &'static str,
    pub commentary: &'static str,
}

fn entry(table: &'static [Entry], slug: &str) -> Option<&'static Entry> {
    table.iter().find(|e| e.slug == slug)
}

fn aspect(table: &'static [AspectEntry], kind: &str) -> Option<&'static AspectEntry> {
    table.iter().find(|a| a.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(en.aspect_name("trine", "Sun", "Moon"), "Sun trine Moon");
        assert_eq!(en.anonymous(), "Anonymous");
    }

    #[test]
    fn russian_has_names_and_terms_for_every_element() {
        let ru = Locale::Ru;
        assert_eq!(ru.planet_name("sun"), "Солнце");
        assert_eq!(ru.sign_name("cancer"), "Рак");
        assert!(ru.house_name(5).contains("дом"));
        assert!(ru.planet_terms("sun").contains(&"солнце"));
        assert!(ru.sign_terms("cancer").contains(&"рак"));
        assert!(ru.house_terms(5).iter().any(|t| t.contains("дом")));
        assert!(!ru.aspect_match_words("trine").is_empty());
    }
}
