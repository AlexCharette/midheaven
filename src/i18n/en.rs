//! English vocabulary. These strings must stay byte-identical to the legacy
//! hardcoded values (`chart::catalog` names, the old `route::lexicon` term
//! lists) so English output never shifts — the tests in `super` guard it.

use super::{AspectEntry, Entry, HouseEntry, LocaleTable, PdfChrome};

pub static TABLE: LocaleTable = LocaleTable {
    planets: PLANETS,
    signs: SIGNS,
    houses: HOUSES,
    aspects: ASPECTS,
    system: "Whole Sign",
    zodiac: "Tropical",
    anonymous: "Anonymous",
    pdf: PdfChrome {
        nativity_of: "The Nativity of",
        prepared_by: "Prepared by",
        index_of_elements: "Index of Elements",
        commentary: "Commentary",
    },
};

// Terms mirror the old `planet_terms`: the slug itself, plus "rising" for the
// Ascendant. The slug doubled as the English match term historically.
static PLANETS: &[Entry] = &[
    Entry { slug: "sun", name: "Sun", terms: &["sun"] },
    Entry { slug: "moon", name: "Moon", terms: &["moon"] },
    Entry { slug: "mercury", name: "Mercury", terms: &["mercury"] },
    Entry { slug: "venus", name: "Venus", terms: &["venus"] },
    Entry { slug: "mars", name: "Mars", terms: &["mars"] },
    Entry { slug: "jupiter", name: "Jupiter", terms: &["jupiter"] },
    Entry { slug: "saturn", name: "Saturn", terms: &["saturn"] },
    Entry { slug: "uranus", name: "Uranus", terms: &["uranus"] },
    Entry { slug: "neptune", name: "Neptune", terms: &["neptune"] },
    Entry { slug: "pluto", name: "Pluto", terms: &["pluto"] },
    Entry { slug: "ascendant", name: "Ascendant", terms: &["ascendant", "rising"] },
];

// Sign terms were historically just the slug.
static SIGNS: &[Entry] = &[
    Entry { slug: "aries", name: "Aries", terms: &["aries"] },
    Entry { slug: "taurus", name: "Taurus", terms: &["taurus"] },
    Entry { slug: "gemini", name: "Gemini", terms: &["gemini"] },
    Entry { slug: "cancer", name: "Cancer", terms: &["cancer"] },
    Entry { slug: "leo", name: "Leo", terms: &["leo"] },
    Entry { slug: "virgo", name: "Virgo", terms: &["virgo"] },
    Entry { slug: "libra", name: "Libra", terms: &["libra"] },
    Entry { slug: "scorpio", name: "Scorpio", terms: &["scorpio"] },
    Entry { slug: "sagittarius", name: "Sagittarius", terms: &["sagittarius"] },
    Entry { slug: "capricorn", name: "Capricorn", terms: &["capricorn"] },
    Entry { slug: "aquarius", name: "Aquarius", terms: &["aquarius"] },
    Entry { slug: "pisces", name: "Pisces", terms: &["pisces"] },
];

// Terms mirror the old house-term construction: the lowercased name, the
// "{n}{ordinal-suffix} house" form, and "house {n}".
static HOUSES: &[HouseEntry] = &[
    HouseEntry { name: "First House", terms: &["first house", "1st house", "house 1"] },
    HouseEntry { name: "Second House", terms: &["second house", "2nd house", "house 2"] },
    HouseEntry { name: "Third House", terms: &["third house", "3rd house", "house 3"] },
    HouseEntry { name: "Fourth House", terms: &["fourth house", "4th house", "house 4"] },
    HouseEntry { name: "Fifth House", terms: &["fifth house", "5th house", "house 5"] },
    HouseEntry { name: "Sixth House", terms: &["sixth house", "6th house", "house 6"] },
    HouseEntry { name: "Seventh House", terms: &["seventh house", "7th house", "house 7"] },
    HouseEntry { name: "Eighth House", terms: &["eighth house", "8th house", "house 8"] },
    HouseEntry { name: "Ninth House", terms: &["ninth house", "9th house", "house 9"] },
    HouseEntry { name: "Tenth House", terms: &["tenth house", "10th house", "house 10"] },
    HouseEntry { name: "Eleventh House", terms: &["eleventh house", "11th house", "house 11"] },
    HouseEntry { name: "Twelfth House", terms: &["twelfth house", "12th house", "house 12"] },
];

// `word` doubles as the catalog kind (the old aspect name composed
// "{a} {kind} {b}"); `match_words` mirror the old `aspect_words`.
static ASPECTS: &[AspectEntry] = &[
    AspectEntry {
        kind: "conjunction",
        word: "conjunction",
        match_words: &["conjunct", "conjunction", "conjoined", "conjoins"],
    },
    AspectEntry {
        kind: "sextile",
        word: "sextile",
        match_words: &["sextile", "sextiles", "sextiling"],
    },
    AspectEntry {
        kind: "square",
        word: "square",
        match_words: &["square", "squares", "squaring"],
    },
    AspectEntry {
        kind: "trine",
        word: "trine",
        match_words: &["trine", "trines", "trining"],
    },
    AspectEntry {
        kind: "opposition",
        word: "opposition",
        match_words: &["opposition", "opposite", "opposes", "opposing"],
    },
];
