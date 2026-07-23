//! The calculation variants the app exposes, and the one place a wire code maps
//! to a `xalen` type. Both the CLI and the desktop command layer resolve a
//! user's string choice through here, and `i18n` keys its labels on the same
//! codes — so adding a system or ayanamsa is a single line in one of the tables
//! below. Kebab-case codes match the `i18n::Locale` string-code precedent.

use xalen_ayanamsa::Ayanamsa;
use xalen_houses::HouseSystem;

/// House systems offered in the UI — the common Western set. All are backed by
/// `xalen-houses`' validated cusp trigonometry; the first entry is the default.
pub const HOUSE_SYSTEMS: &[(&str, HouseSystem)] = &[
    ("whole-sign", HouseSystem::WholeSign),
    ("placidus", HouseSystem::Placidus),
    ("koch", HouseSystem::Koch),
    ("equal", HouseSystem::Equal),
    ("regiomontanus", HouseSystem::Regiomontanus),
    ("campanus", HouseSystem::Campanus),
    ("porphyry", HouseSystem::Porphyry),
];

/// Ayanamsas offered when the sidereal zodiac is chosen; the first is the
/// default. The list is deliberately short — the registry makes it one line to
/// add more from `xalen_ayanamsa::Ayanamsa`.
pub const AYANAMSAS: &[(&str, Ayanamsa)] = &[
    ("lahiri", Ayanamsa::Lahiri),
    ("fagan-bradley", Ayanamsa::FaganBradley),
    ("kp", Ayanamsa::KPKrishnamurti),
    ("raman", Ayanamsa::Raman),
    ("true-chitra", Ayanamsa::TrueChitra),
];

/// The default house system — matches the app's historical behaviour, so an
/// absent choice reproduces the old output exactly.
pub const DEFAULT_HOUSE_SYSTEM: HouseSystem = HouseSystem::WholeSign;

/// Resolve a house-system code; unknown or empty → the default (`WholeSign`).
pub fn house_system(code: &str) -> HouseSystem {
    let code = code.trim();
    HOUSE_SYSTEMS
        .iter()
        .find(|(c, _)| *c == code)
        .map_or(DEFAULT_HOUSE_SYSTEM, |(_, s)| *s)
}

/// The wire code for a house system; the default's code for anything unlisted.
pub fn house_code(system: HouseSystem) -> &'static str {
    HOUSE_SYSTEMS
        .iter()
        .find(|(_, s)| *s == system)
        .map_or("whole-sign", |(c, _)| *c)
}

/// Resolve an ayanamsa code; unknown or empty → the default (`Lahiri`).
pub fn ayanamsa(code: &str) -> Ayanamsa {
    let code = code.trim();
    AYANAMSAS
        .iter()
        .find(|(c, _)| *c == code)
        .map_or(Ayanamsa::Lahiri, |(_, a)| *a)
}

/// The wire code for an ayanamsa; the default's code for anything unlisted.
pub fn ayanamsa_code(a: Ayanamsa) -> &'static str {
    AYANAMSAS
        .iter()
        .find(|(_, x)| *x == a)
        .map_or("lahiri", |(c, _)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn house_codes_round_trip() {
        for &(code, system) in HOUSE_SYSTEMS {
            assert_eq!(house_system(code), system);
            assert_eq!(house_code(system), code);
        }
        // Unknown / empty falls back to the historical default.
        assert_eq!(house_system("nonsense"), HouseSystem::WholeSign);
        assert_eq!(house_system(""), HouseSystem::WholeSign);
    }

    #[test]
    fn ayanamsa_codes_round_trip() {
        for &(code, a) in AYANAMSAS {
            assert_eq!(ayanamsa(code), a);
            assert_eq!(ayanamsa_code(a), code);
        }
        assert_eq!(ayanamsa("nonsense"), Ayanamsa::Lahiri);
    }
}
