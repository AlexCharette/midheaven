//! Fixed symbol tables: planets, signs, houses, and aspect definitions.
//! Ids here are the source of the tag-id vocabulary (`planet:sun`, `sign:leo`,
//! `house:5`, `aspect:sun-moon`).

use xalen_ephem::Body as XBody;

/// (body, tag-id short name, glyph, display name) — the ten chart bodies.
pub const PLANETS: &[(XBody, &str, &str, &str)] = &[
    (XBody::Sun, "sun", "☉", "Sun"),
    (XBody::Moon, "moon", "☽", "Moon"),
    (XBody::Mercury, "mercury", "☿", "Mercury"),
    (XBody::Venus, "venus", "♀", "Venus"),
    (XBody::Mars, "mars", "♂", "Mars"),
    (XBody::Jupiter, "jupiter", "♃", "Jupiter"),
    (XBody::Saturn, "saturn", "♄", "Saturn"),
    (XBody::Uranus, "uranus", "♅", "Uranus"),
    (XBody::Neptune, "neptune", "♆", "Neptune"),
    (XBody::Pluto, "pluto", "♇", "Pluto"),
];

/// (tag-id short name, glyph, display name, classical element),
/// zodiac order from 0° Aries — the element cycle is fire/earth/air/water.
pub const SIGNS_ALL: &[(&str, &str, &str, &str)] = &[
    ("aries", "♈", "Aries", "fire"),
    ("taurus", "♉", "Taurus", "earth"),
    ("gemini", "♊", "Gemini", "air"),
    ("cancer", "♋", "Cancer", "water"),
    ("leo", "♌", "Leo", "fire"),
    ("virgo", "♍", "Virgo", "earth"),
    ("libra", "♎", "Libra", "air"),
    ("scorpio", "♏", "Scorpio", "water"),
    ("sagittarius", "♐", "Sagittarius", "fire"),
    ("capricorn", "♑", "Capricorn", "earth"),
    ("aquarius", "♒", "Aquarius", "air"),
    ("pisces", "♓", "Pisces", "water"),
];

/// (roman numeral label, display name), house 1 first.
pub const HOUSE_NAMES: &[(&str, &str)] = &[
    ("I", "First House"),
    ("II", "Second House"),
    ("III", "Third House"),
    ("IV", "Fourth House"),
    ("V", "Fifth House"),
    ("VI", "Sixth House"),
    ("VII", "Seventh House"),
    ("VIII", "Eighth House"),
    ("IX", "Ninth House"),
    ("X", "Tenth House"),
    ("XI", "Eleventh House"),
    ("XII", "Twelfth House"),
];

/// (kind, glyph, exact angle, orb, nature) — the five Ptolemaic aspects. The
/// kind doubles as the display word; the nature drives the classic chord
/// coloring (harmonious = ease and innate strengths, challenging = friction
/// that drives growth, conjunction = neutral blending).
pub const ASPECT_TYPES: &[(&str, &str, f64, f64, &str)] = &[
    ("conjunction", "☌", 0.0, 8.0, "neutral"),
    ("sextile", "⚹", 60.0, 5.0, "harmonious"),
    ("square", "□", 90.0, 7.0, "challenging"),
    ("trine", "△", 120.0, 7.0, "harmonious"),
    ("opposition", "☍", 180.0, 8.0, "challenging"),
];
