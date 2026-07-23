//! Stage 2 — compute the chart: birth data → `ChartData` chart fields.
//! Tropical zodiac, Whole Sign houses. Pure analytic ephemeris
//! (VSOP87A + ELP2000-82 via `xalen-ephem`), no data files, fully offline.

pub mod catalog;
pub mod systems;

use crate::contract::{Aspect, Axes, Body, ChartData, HouseRef, Meta, Ref};
use crate::i18n::Locale;
use catalog::{ASPECT_TYPES, HOUSE_NAMES, PLANETS, SIGNS_ALL};
use chrono::{Datelike, NaiveDate, NaiveTime, TimeZone, Timelike};
use chrono_tz::Tz;
use xalen_ayanamsa::{Ayanamsa, tropical_to_sidereal};
use xalen_coords::mean_obliquity;
use xalen_ephem::Almanac;
use xalen_houses::{GeoLocation, HouseCusps, HouseSystem, compute_houses};
use xalen_time::{
    CalendarSystem, DAYS_PER_JULIAN_CENTURY, DeltaTModel, J2000_JD, JdUT1, calendar_to_jd,
};

pub struct BirthInput {
    pub name: String,
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub lat: f64,
    pub lon: f64,
    pub tz: Tz,
    pub place: String,
    /// The language the reading is in — drives element names and, downstream,
    /// the router's match terms (recorded on `meta.locale`).
    pub locale: Locale,
    /// The house division to compute (default Whole Sign).
    pub house_system: HouseSystem,
    /// `None` = tropical zodiac; `Some(a)` = sidereal, shifting every longitude
    /// and house cusp by that ayanamsa.
    pub ayanamsa: Option<Ayanamsa>,
}

fn norm360(x: f64) -> f64 {
    x.rem_euclid(360.0)
}

/// Angular separation of two ecliptic longitudes, in [0, 180].
pub fn separation(a: f64, b: f64) -> f64 {
    let d = norm360(a - b);
    d.min(360.0 - d)
}

/// Parse a birth time, accepting HH:MM:SS or HH:MM (seconds default to 00).
/// The one time-input rule shared by every frontend.
pub fn parse_time(s: &str) -> Result<NaiveTime, String> {
    s.parse()
        .or_else(|_| format!("{s}:00").parse())
        .map_err(|e| format!("invalid time {s:?}: {e}"))
}

/// The spine: each stage is a named step; the data flows top to bottom.
/// Returns the chart plus any non-fatal warnings (e.g. a DST-ambiguous birth
/// time) for the caller to surface. [`compute_chart`] is the common form that
/// drops them.
pub fn compute_chart_reporting(input: &BirthInput) -> Result<(ChartData, Vec<String>), String> {
    let loc = input.locale;
    let (jd_ut1, warning) = birth_moment(input)?;
    // Terrestrial Time drives both the obliquity (inside `natal_houses`) and the
    // ayanamsa epoch, so compute it once and thread it through.
    let jd_tt = jd_ut1.to_tt(&DeltaTModel::StephensonMorrisonHohenkerk2016).0;
    let mut cusps = natal_houses(jd_ut1, jd_tt, input.lat, input.lon, input.house_system)?;
    // Sidereal: shift every cusp and angle by the ayanamsa. Planet longitudes
    // get the same shift below, so house assignment stays consistent.
    if let Some(ay) = input.ayanamsa {
        cusps = cusps.to_sidereal(ay.compute(jd_tt));
    }
    let mut warnings: Vec<String> = warning.into_iter().collect();
    if cusps.fallback_used {
        // Placidus/Koch are undefined near the poles; the crate substitutes
        // Porphyry. Surface it rather than silently returning other cusps.
        warnings.push(format!(
            "{} houses are undefined at this latitude; the chart uses Porphyry cusps instead.",
            input.house_system
        ));
    }
    let mut planets = planet_positions(jd_ut1, jd_tt, &cusps, loc, input.ayanamsa)?;
    let aspects = detect_aspects(&planets, loc);
    let asc = norm360(cusps.ascendant.to_degrees());
    planets.push(ascendant_point(asc, loc));
    let chart = ChartData {
        meta: birth_meta(input),
        axes: Axes { asc, mc: norm360(cusps.mc.to_degrees()) },
        house_cusps: (0..12).map(|i| cusps.cusp_deg(i)).collect(),
        planets,
        signs: sign_refs(loc),
        houses: house_refs(loc),
        aspects,
        excerpts: Vec::new(),
    };
    Ok((chart, warnings))
}

/// Compute the chart, dropping any non-fatal warnings — the common form used
/// by `astro chart` and tests. Pipeline entry points that surface warnings use
/// [`compute_chart_reporting`].
pub fn compute_chart(input: &BirthInput) -> Result<ChartData, String> {
    compute_chart_reporting(input).map(|(chart, _)| chart)
}

/// The birth instant, resolved: local civil time → UTC (via the historical
/// IANA tz database) → the UT1 Julian Day the astronomy runs on. Returns a
/// warning (not stderr) when the civil time is DST-ambiguous.
fn birth_moment(input: &BirthInput) -> Result<(JdUT1, Option<String>), String> {
    let naive = input.date.and_time(input.time);
    let mut warning = None;
    let local = match input.tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(early, _) => {
            warning =
                Some(format!("{naive} is ambiguous in {} (DST fold); using the earlier offset", input.tz));
            early
        }
        chrono::LocalResult::None => {
            return Err(format!("{naive} does not exist in {} (DST gap)", input.tz));
        }
    };
    let utc = local.with_timezone(&chrono::Utc);
    let frac_hour =
        utc.hour() as f64 + utc.minute() as f64 / 60.0 + utc.second() as f64 / 3600.0;
    let jd_ut1 = calendar_to_jd(
        utc.year(),
        utc.month(),
        utc.day(),
        frac_hour,
        CalendarSystem::ProlepticGregorian,
    );
    Ok((jd_ut1, warning))
}

/// Cusps and angles for the birth place in the chosen `system`, using the mean
/// obliquity of date (tropical frame; the caller shifts to sidereal if asked).
fn natal_houses(
    jd_ut1: JdUT1,
    jd_tt: f64,
    lat: f64,
    lon: f64,
    system: HouseSystem,
) -> Result<HouseCusps, String> {
    let t = (jd_tt - J2000_JD) / DAYS_PER_JULIAN_CENTURY;
    let eps = mean_obliquity(t);
    let location = GeoLocation::try_new(lat, lon)
        .ok_or_else(|| format!("invalid coordinates: lat {lat} lon {lon}"))?;
    Ok(compute_houses(jd_ut1.0, &location, eps, system))
}

/// Geocentric ecliptic longitudes for the ten catalog bodies, each placed in
/// its house. When `ayanamsa` is `Some`, longitudes are shifted to the sidereal
/// frame — the same shift the cusps received, so house assignment is unchanged.
fn planet_positions(
    jd_ut1: JdUT1,
    jd_tt: f64,
    cusps: &HouseCusps,
    loc: Locale,
    ayanamsa: Option<Ayanamsa>,
) -> Result<Vec<Body>, String> {
    let almanac = Almanac::default_vedic(); // default VSOP87 provider chain
    PLANETS
        .iter()
        .map(|&(body, id, glyph, _name)| {
            let pos = almanac
                .geocentric_ecliptic(body, jd_ut1)
                .map_err(|e| format!("ephemeris error for planet:{id}: {e:?}"))?;
            // Sidereal longitude (radians) when an ayanamsa is set; used for both
            // the reported degree and the house lookup, keeping them coherent.
            let lon_rad = match ayanamsa {
                Some(ay) => tropical_to_sidereal(pos.longitude, &ay, jd_tt),
                None => pos.longitude,
            };
            Ok(Body {
                id: format!("planet:{id}"),
                glyph: glyph.to_string(),
                name: loc.planet_name(id).to_string(),
                lon: norm360(lon_rad.to_degrees()),
                house: cusps.planet_in_house(lon_rad) as u8,
            })
        })
        .collect()
}

/// Ptolemaic aspects among the planet bodies (not the ASC point), by the
/// catalog's standard orbs — at most one aspect kind per pair. `planets`
/// must be the [`planet_positions`] output: same order as [`PLANETS`], whose
/// short names build the aspect ids.
fn detect_aspects(planets: &[Body], loc: Locale) -> Vec<Aspect> {
    debug_assert_eq!(planets.len(), PLANETS.len());
    let mut aspects = Vec::new();
    for i in 0..planets.len() {
        for j in (i + 1)..planets.len() {
            let sep = separation(planets[i].lon, planets[j].lon);
            for &(kind, glyph, angle, orb, nature) in ASPECT_TYPES {
                if (sep - angle).abs() <= orb {
                    let (a_short, b_short) = (PLANETS[i].1, PLANETS[j].1);
                    aspects.push(Aspect {
                        id: format!("aspect:{a_short}-{b_short}"),
                        glyph: glyph.to_string(),
                        name: loc.aspect_name(kind, &planets[i].name, &planets[j].name),
                        a: planets[i].id.clone(),
                        b: planets[j].id.clone(),
                        nature: nature.to_string(),
                        orb: (sep - angle).abs(),
                        kind,
                    });
                    break;
                }
            }
        }
    }
    aspects
}

/// The Ascendant rendered as a chart point, per the contract.
fn ascendant_point(asc_deg: f64, loc: Locale) -> Body {
    Body {
        id: "planet:ascendant".to_string(),
        glyph: "AC".to_string(),
        name: loc.planet_name("ascendant").to_string(),
        lon: asc_deg,
        house: 1,
    }
}

fn sign_refs(loc: Locale) -> Vec<Ref> {
    SIGNS_ALL
        .iter()
        .map(|&(id, glyph, _name, element)| Ref {
            id: format!("sign:{id}"),
            glyph: glyph.to_string(),
            name: loc.sign_name(id).to_string(),
            element: element.to_string(),
        })
        .collect()
}

fn house_refs(loc: Locale) -> Vec<HouseRef> {
    // The roman-numeral label stays language-neutral (catalog); only the name
    // is localized.
    HOUSE_NAMES
        .iter()
        .enumerate()
        .map(|(i, &(label, _name))| HouseRef {
            id: format!("house:{}", i + 1),
            label: label.to_string(),
            name: loc.house_name(i + 1).to_string(),
        })
        .collect()
}

fn birth_meta(input: &BirthInput) -> Meta {
    let loc = input.locale;
    let house_code = systems::house_code(input.house_system);
    Meta {
        name: input.name.clone(),
        // The zone/offset stays out of `born`: it drives the local→UT
        // conversion but is noise to the reader.
        born: format!("{} {}", input.date, input.time.format("%H:%M")),
        place: input.place.clone(),
        system: loc.house_system_label(house_code).to_string(),
        zodiac: loc.zodiac_label_for(input.ayanamsa),
        house_system: house_code.to_string(),
        ayanamsa: input.ayanamsa.map(|a| systems::ayanamsa_code(a).to_string()),
        locale: loc.code().to_string(),
        astrologer: None, // branding is a frontend concern (desktop prefs)
        logo: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn birth(date: &str, time: &str, lat: f64, lon: f64, tz: &str) -> BirthInput {
        BirthInput {
            name: "Test".into(),
            date: date.parse().unwrap(),
            time: time.parse().unwrap(),
            lat,
            lon,
            tz: tz.parse().unwrap(),
            place: "Test".into(),
            locale: Locale::En,
            house_system: HouseSystem::WholeSign,
            ayanamsa: None,
        }
    }

    /// Golden test: at the March 2000 equinox (2000-03-20 07:35 UT) the Sun's
    /// apparent ecliptic longitude is 0° by definition, within arcminutes.
    #[test]
    fn sun_at_equinox_is_zero_aries() {
        let input = birth("2000-03-20", "07:35:00", 0.0, 0.0, "UTC");
        let chart = compute_chart(&input).unwrap();
        let sun = &chart.planets[0];
        let dist = separation(sun.lon, 0.0);
        assert!(dist < 0.05, "Sun at equinox should be ~0° Aries, got {}", sun.lon);
    }

    #[test]
    fn whole_sign_cusps_are_sign_boundaries() {
        let input = birth("1990-07-13", "14:30:00", 52.52, 13.405, "Europe/Berlin");
        let chart = compute_chart(&input).unwrap();
        // Every Whole Sign cusp is a multiple of 30° (within float rounding).
        for c in &chart.house_cusps {
            let nearest = (c / 30.0).round() * 30.0;
            assert!((c - nearest).abs() < 1e-6, "cusp {c} not a sign boundary");
        }
        // First cusp is 0° of the Ascendant's sign.
        let asc_sign_start = (chart.axes.asc / 30.0).floor() * 30.0;
        assert!(separation(chart.house_cusps[0], asc_sign_start) < 1e-6);
    }

    #[test]
    fn placidus_cusps_are_quadrant_not_sign_boundaries() {
        let mut input = birth("1990-07-13", "14:30:00", 52.52, 13.405, "Europe/Berlin");
        input.house_system = HouseSystem::Placidus;
        let chart = compute_chart(&input).unwrap();
        // A quadrant system at mid-latitude yields intermediate cusps: at least
        // one is clearly off a 30° boundary (unlike Whole Sign).
        let off_boundary = chart.house_cusps.iter().any(|c| {
            let nearest = (c / 30.0).round() * 30.0;
            (c - nearest).abs() > 0.5
        });
        assert!(off_boundary, "Placidus cusps unexpectedly all on sign boundaries");
        // The Ascendant still coincides with the first house cusp.
        assert!(separation(chart.axes.asc, chart.house_cusps[0]) < 1e-6);
        assert_eq!(chart.meta.system, "Placidus");
        assert_eq!(chart.meta.house_system, "placidus");
    }

    #[test]
    fn sidereal_shifts_every_longitude_by_the_ayanamsa() {
        let tropical = birth("2000-01-01", "12:00:00", 0.0, 0.0, "UTC");
        let mut sidereal = birth("2000-01-01", "12:00:00", 0.0, 0.0, "UTC");
        sidereal.ayanamsa = Some(Ayanamsa::Lahiri);
        let ct = compute_chart(&tropical).unwrap();
        let cs = compute_chart(&sidereal).unwrap();
        // Lahiri near J2000 is ≈ 23.85°; each sidereal longitude is the tropical
        // one minus the ayanamsa (mod 360), bodies and the ASC point alike.
        for (t, s) in ct.planets.iter().zip(cs.planets.iter()) {
            let shift = norm360(t.lon - s.lon);
            assert!((shift - 23.85).abs() < 0.3, "{}: shift {shift}", t.id);
        }
        assert_eq!(cs.meta.ayanamsa.as_deref(), Some("lahiri"));
        assert!(cs.meta.zodiac.starts_with("Sidereal"), "zodiac {}", cs.meta.zodiac);
        // Tropical charts record no ayanamsa.
        assert_eq!(ct.meta.ayanamsa, None);
    }

    #[test]
    fn historical_dst_offset_applied() {
        // Berlin, July 1990 → CEST (UTC+2): 14:30 local = 12:30 UT. The same
        // instant expressed in either zone must give the same Ascendant —
        // the Asc moves ~1° per 4 minutes, so a wrong offset shifts it ~30°.
        let local = birth("1990-07-13", "14:30:00", 52.52, 13.405, "Europe/Berlin");
        let utc = birth("1990-07-13", "12:30:00", 52.52, 13.405, "UTC");
        let chart = compute_chart(&local).unwrap();
        let same_instant = compute_chart(&utc).unwrap();
        assert!(
            separation(chart.axes.asc, same_instant.axes.asc) < 1e-6,
            "asc {} vs {}",
            chart.axes.asc,
            same_instant.axes.asc
        );
        // The zone stays out of the displayed birth data.
        assert_eq!(chart.meta.born, "1990-07-13 14:30");
        // Sun in mid-July is in Cancer [90°, 120°).
        let sun = &chart.planets[0];
        assert!((90.0..120.0).contains(&sun.lon), "Sun lon {}", sun.lon);
    }

    #[test]
    fn vocab_contains_all_categories() {
        let input = birth("1990-07-13", "14:30:00", 52.52, 13.405, "Europe/Berlin");
        let chart = compute_chart(&input).unwrap();
        let vocab = chart.vocab();
        assert!(vocab.contains("planet:sun"));
        assert!(vocab.contains("planet:ascendant"));
        assert!(vocab.contains("sign:leo"));
        assert!(vocab.contains("house:5"));
        assert!(vocab.iter().any(|t| t.starts_with("aspect:")));
    }
}
