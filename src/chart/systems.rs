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

/// The wire codes a calculation falls back to when nothing was asked for and
/// nothing is preferred — the app's historical behaviour, so an absent choice
/// reproduces the old output exactly.
///
/// The one home for these three strings. They used to appear 64 times across
/// the Rust core, the CLI, the command layer and four Svelte components, and
/// the webview now reads them from here through `calculation_defaults`.
pub const DEFAULTS: Codes<'static> =
    Codes { house_system: Some("whole-sign"), zodiac: Some("tropical"), ayanamsa: Some("lahiri") };

/// The default house system.
pub const DEFAULT_HOUSE_SYSTEM: HouseSystem = HouseSystem::WholeSign;

/// Resolve a house-system code.
///
/// Refuses an unknown one. It used to return `WholeSign` for anything it did not
/// recognize, so a typo, a stale preference and a deliberate choice were the
/// same input: the chart came back claiming `whole-sign`, the reading view's
/// effect snapped the selector to match, and nothing was said.
pub fn house_system(code: &str) -> Result<HouseSystem, String> {
    let code = code.trim();
    HOUSE_SYSTEMS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, s)| *s)
        .ok_or_else(|| format!("unknown house system {code:?}"))
}

/// The wire code for a house system; the default's code for anything unlisted.
pub fn house_code(system: HouseSystem) -> &'static str {
    HOUSE_SYSTEMS
        .iter()
        .find(|(_, s)| *s == system)
        .map_or("whole-sign", |(c, _)| *c)
}

/// Resolve an ayanamsa code. Refuses an unknown one, for the same reason
/// [`house_system`] does.
pub fn ayanamsa(code: &str) -> Result<Ayanamsa, String> {
    let code = code.trim();
    AYANAMSAS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, a)| *a)
        .ok_or_else(|| format!("unknown ayanamsa {code:?}"))
}

/// The wire code for an ayanamsa; the default's code for anything unlisted.
pub fn ayanamsa_code(a: Ayanamsa) -> &'static str {
    AYANAMSAS
        .iter()
        .find(|(_, x)| *x == a)
        .map_or("lahiri", |(c, _)| *c)
}

/// A calculation as wire codes, each absent when the tier did not state one.
/// One tier of the ladder [`resolve`] walks.
#[derive(Debug, Clone, Copy, Default)]
pub struct Codes<'a> {
    pub house_system: Option<&'a str>,
    pub zodiac: Option<&'a str>,
    pub ayanamsa: Option<&'a str>,
}

impl<'a> Codes<'a> {
    /// Blank strings are the same as absent — a form field left empty has not
    /// chosen anything.
    pub fn new(
        house_system: Option<&'a str>,
        zodiac: Option<&'a str>,
        ayanamsa: Option<&'a str>,
    ) -> Codes<'a> {
        let present = |s: Option<&'a str>| s.map(str::trim).filter(|s| !s.is_empty());
        Codes {
            house_system: present(house_system),
            zodiac: present(zodiac),
            ayanamsa: present(ayanamsa),
        }
    }
}

/// A resolved calculation, ready for `BirthInput`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calculation {
    pub house_system: HouseSystem,
    /// `None` = tropical; `Some(a)` = sidereal under that ayanamsa.
    pub ayanamsa: Option<Ayanamsa>,
}

/// The two zodiacs. Unlike the house systems and ayanamsas there is no table to
/// look up — there are two of them — but they are still wire codes, and a code
/// the app does not know still has to be refused rather than guessed at.
pub const ZODIACS: &[&str] = &["tropical", "sidereal"];

/// Whether a zodiac code means sidereal.
///
/// Refuses anything that is neither, the same stance [`house_system`] and
/// [`ayanamsa`] take. It used to return a bare `bool`, which meant an unknown
/// code was silently tropical here — and its one other caller then wrote
/// `!is_sidereal(z) && z != "tropical"` to catch that, a comparison that
/// trimmed and case-folded on one side and not the other, so `" Tropical "` was
/// refused while `" Sidereal "` passed.
pub fn is_sidereal(zodiac: &str) -> Result<bool, String> {
    let code = zodiac.trim();
    if code.eq_ignore_ascii_case("sidereal") {
        Ok(true)
    } else if code.eq_ignore_ascii_case("tropical") {
        Ok(false)
    } else {
        Err(format!("unknown zodiac {code:?}"))
    }
}

/// The three-tier rule, once: what was `asked` for, else what is `preferred`,
/// else [`DEFAULTS`].
///
/// It existed in four shapes — the build command's `.or(pref).unwrap_or(lit)`
/// chains, the preview command's ladder with no preference tier at all, and two
/// more on the frontend gated on different conditions. The ayanamsa only matters
/// under a sidereal zodiac, so a tropical calculation never resolves it and an
/// unknown one beside `tropical` is therefore not an error.
pub fn resolve(asked: Codes, preferred: Codes) -> Result<Calculation, String> {
    fn pick<'a>(a: Option<&'a str>, p: Option<&'a str>, d: Option<&'a str>) -> &'a str {
        a.or(p).or(d).expect("DEFAULTS states every code")
    }
    let zodiac = pick(asked.zodiac, preferred.zodiac, DEFAULTS.zodiac);
    Ok(Calculation {
        house_system: house_system(pick(
            asked.house_system,
            preferred.house_system,
            DEFAULTS.house_system,
        ))?,
        ayanamsa: if is_sidereal(zodiac)? {
            Some(ayanamsa(pick(asked.ayanamsa, preferred.ayanamsa, DEFAULTS.ayanamsa))?)
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn house_codes_round_trip() {
        for &(code, system) in HOUSE_SYSTEMS {
            assert_eq!(house_system(code).unwrap(), system);
            assert_eq!(house_code(system), code);
        }
    }

    #[test]
    fn ayanamsa_codes_round_trip() {
        for &(code, a) in AYANAMSAS {
            assert_eq!(ayanamsa(code).unwrap(), a);
            assert_eq!(ayanamsa_code(a), code);
        }
    }

    /// An unknown code used to become Whole Sign in silence, so a typo, a stale
    /// preference and a real choice were indistinguishable.
    #[test]
    fn an_unknown_code_is_refused_and_named() {
        let err = house_system("nonsense").unwrap_err();
        assert!(err.contains("nonsense"), "{err}");
        // Matching is exact — the codes are a wire format, not prose.
        assert!(house_system("Placidus").is_err(), "codes are case-sensitive");
        assert!(house_system("").is_err());
        assert!(ayanamsa("nonsense").unwrap_err().contains("nonsense"));
        // Surrounding space is forgiven; that is transport, not intent.
        assert_eq!(house_system("  placidus  ").unwrap(), HouseSystem::Placidus);
    }

    #[test]
    fn the_defaults_are_codes_the_resolvers_accept() {
        assert!(house_system(DEFAULTS.house_system.unwrap()).is_ok());
        assert!(ayanamsa(DEFAULTS.ayanamsa.unwrap()).is_ok());
        assert_eq!(house_system(DEFAULTS.house_system.unwrap()).unwrap(), DEFAULT_HOUSE_SYSTEM);
        assert!(!is_sidereal(DEFAULTS.zodiac.unwrap()).unwrap());
    }

    /// Both codes are read the same way. The asymmetry this replaced accepted
    /// `" Sidereal "` and refused `" Tropical "`.
    #[test]
    fn either_zodiac_is_recognized_however_it_arrives() {
        for spelling in ["sidereal", "  Sidereal  ", "SIDEREAL"] {
            assert!(is_sidereal(spelling).unwrap(), "{spelling:?} is sidereal");
        }
        for spelling in ["tropical", "  Tropical  ", "TROPICAL"] {
            assert!(!is_sidereal(spelling).unwrap(), "{spelling:?} is not sidereal");
        }
    }

    #[test]
    fn an_unknown_zodiac_is_refused_rather_than_read_as_tropical() {
        for bad in ["", "  ", "nonsense", "siderial"] {
            assert!(is_sidereal(bad).is_err(), "{bad:?} should be refused");
        }
        assert!(is_sidereal("bogus").unwrap_err().contains("bogus"));
        // And it refuses the whole calculation, as an unknown house system does.
        let err = resolve(Codes::new(None, Some("bogus"), None), Codes::default()).unwrap_err();
        assert!(err.contains("bogus"), "{err}");
    }

    #[test]
    fn the_two_zodiac_codes_are_the_ones_the_resolver_accepts() {
        for code in ZODIACS {
            assert!(is_sidereal(code).is_ok(), "{code:?}");
        }
        assert!(ZODIACS.contains(&DEFAULTS.zodiac.unwrap()));
    }

    #[test]
    fn nothing_asked_and_nothing_preferred_is_the_historical_calculation() {
        let c = resolve(Codes::default(), Codes::default()).unwrap();
        assert_eq!(c.house_system, HouseSystem::WholeSign);
        assert_eq!(c.ayanamsa, None, "tropical resolves no ayanamsa");
    }

    #[test]
    fn what_was_asked_beats_what_is_preferred_beats_the_default() {
        let preferred = Codes::new(Some("koch"), Some("sidereal"), Some("raman"));

        // Nothing asked: the preference wins.
        let c = resolve(Codes::default(), preferred).unwrap();
        assert_eq!(c.house_system, HouseSystem::Koch);
        assert_eq!(c.ayanamsa, Some(Ayanamsa::Raman));

        // Asked: it wins, field by field.
        let asked = Codes::new(Some("placidus"), None, None);
        let c = resolve(asked, preferred).unwrap();
        assert_eq!(c.house_system, HouseSystem::Placidus, "the asked house system");
        assert_eq!(c.ayanamsa, Some(Ayanamsa::Raman), "the preferred zodiac and ayanamsa");
    }

    /// A form field left empty has not chosen anything — otherwise a blank
    /// would beat a preference.
    #[test]
    fn a_blank_choice_is_no_choice() {
        let preferred = Codes::new(Some("koch"), None, None);
        let c = resolve(Codes::new(Some("   "), Some(""), None), preferred).unwrap();
        assert_eq!(c.house_system, HouseSystem::Koch);
    }

    /// The ayanamsa is only consulted under a sidereal zodiac, so a tropical
    /// calculation never resolves one — and cannot fail on a bad one.
    #[test]
    fn the_ayanamsa_matters_only_when_the_zodiac_is_sidereal() {
        let tropical = resolve(Codes::new(None, Some("tropical"), Some("nonsense")), Codes::default());
        assert_eq!(tropical.unwrap().ayanamsa, None);

        let sidereal = resolve(Codes::new(None, Some("sidereal"), Some("nonsense")), Codes::default());
        assert!(sidereal.unwrap_err().contains("nonsense"));

        let ok = resolve(Codes::new(None, Some("sidereal"), None), Codes::default()).unwrap();
        assert_eq!(ok.ayanamsa, Some(Ayanamsa::Lahiri), "sidereal falls back to the default");
    }

    #[test]
    fn an_unknown_house_system_refuses_the_whole_calculation() {
        let err = resolve(Codes::new(Some("bogus"), None, None), Codes::default()).unwrap_err();
        assert!(err.contains("bogus"), "{err}");
        // Including when it came from a preference rather than a form.
        let err = resolve(Codes::default(), Codes::new(Some("bogus"), None, None)).unwrap_err();
        assert!(err.contains("bogus"), "{err}");
    }
}
