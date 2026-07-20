//! Stage 2 — compute the chart: birth data → `ChartData` chart fields.
//! Tropical zodiac, Whole Sign houses. Pure analytic ephemeris
//! (VSOP87A + ELP2000-82 via `xalen-ephem`), no data files, fully offline.

pub mod catalog;

use crate::contract::{Aspect, Axes, Body, ChartData, HouseRef, Meta, Ref};
use catalog::{ASPECT_TYPES, HOUSE_NAMES, PLANETS, SIGNS_ALL};
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeZone, Timelike};
use chrono_tz::Tz;
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
pub fn compute_chart(input: &BirthInput) -> Result<ChartData, String> {
    let moment = birth_moment(input)?;
    let cusps = natal_houses(moment.jd_ut1, input.lat, input.lon)?;
    let mut planets = planet_positions(moment.jd_ut1, &cusps)?;
    let aspects = detect_aspects(&planets);
    let asc = norm360(cusps.ascendant.to_degrees());
    planets.push(ascendant_point(asc));
    Ok(ChartData {
        meta: birth_meta(input, &moment),
        axes: Axes { asc, mc: norm360(cusps.mc.to_degrees()) },
        house_cusps: (0..12).map(|i| cusps.cusp_deg(i)).collect(),
        planets,
        signs: sign_refs(),
        houses: house_refs(),
        aspects,
        excerpts: Vec::new(),
    })
}

/// The birth instant, resolved: local civil time in its historical timezone
/// plus the UT1 Julian Day the astronomy runs on.
struct BirthMoment {
    local: DateTime<Tz>,
    jd_ut1: JdUT1,
}

/// Local civil time → UTC (via the historical IANA tz database) → Julian Day.
fn birth_moment(input: &BirthInput) -> Result<BirthMoment, String> {
    let naive = input.date.and_time(input.time);
    let local = match input.tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(early, _) => {
            eprintln!("warning: {naive} is ambiguous in {} (DST fold); using earlier offset", input.tz);
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
    Ok(BirthMoment { local, jd_ut1 })
}

/// Whole Sign cusps and angles for the birth place, using the mean obliquity
/// of date.
fn natal_houses(jd_ut1: JdUT1, lat: f64, lon: f64) -> Result<HouseCusps, String> {
    let jd_tt = jd_ut1.to_tt(&DeltaTModel::StephensonMorrisonHohenkerk2016);
    let t = (jd_tt.0 - J2000_JD) / DAYS_PER_JULIAN_CENTURY;
    let eps = mean_obliquity(t);
    let location = GeoLocation::try_new(lat, lon)
        .ok_or_else(|| format!("invalid coordinates: lat {lat} lon {lon}"))?;
    Ok(compute_houses(jd_ut1.0, &location, eps, HouseSystem::WholeSign))
}

/// Geocentric ecliptic longitudes for the ten catalog bodies, each placed in
/// its house.
fn planet_positions(jd_ut1: JdUT1, cusps: &HouseCusps) -> Result<Vec<Body>, String> {
    let almanac = Almanac::default_vedic(); // default VSOP87 provider chain
    PLANETS
        .iter()
        .map(|&(body, id, glyph, name)| {
            let pos = almanac
                .geocentric_ecliptic(body, jd_ut1)
                .map_err(|e| format!("ephemeris error for {name}: {e:?}"))?;
            Ok(Body {
                id: format!("planet:{id}"),
                glyph: glyph.to_string(),
                name: name.to_string(),
                lon: norm360(pos.longitude.to_degrees()),
                house: cusps.planet_in_house(pos.longitude) as u8,
            })
        })
        .collect()
}

/// Ptolemaic aspects among the planet bodies (not the ASC point), by the
/// catalog's standard orbs — at most one aspect kind per pair. `planets`
/// must be the [`planet_positions`] output: same order as [`PLANETS`], whose
/// short names build the aspect ids.
fn detect_aspects(planets: &[Body]) -> Vec<Aspect> {
    debug_assert_eq!(planets.len(), PLANETS.len());
    let mut aspects = Vec::new();
    for i in 0..planets.len() {
        for j in (i + 1)..planets.len() {
            let sep = separation(planets[i].lon, planets[j].lon);
            for &(kind, glyph, angle, orb) in ASPECT_TYPES {
                if (sep - angle).abs() <= orb {
                    let (a_short, b_short) = (PLANETS[i].1, PLANETS[j].1);
                    aspects.push(Aspect {
                        id: format!("aspect:{a_short}-{b_short}"),
                        glyph: glyph.to_string(),
                        name: format!("{} {} {}", planets[i].name, kind, planets[j].name),
                        a: planets[i].id.clone(),
                        b: planets[j].id.clone(),
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
fn ascendant_point(asc_deg: f64) -> Body {
    Body {
        id: "planet:ascendant".to_string(),
        glyph: "AC".to_string(),
        name: "Ascendant".to_string(),
        lon: asc_deg,
        house: 1,
    }
}

fn sign_refs() -> Vec<Ref> {
    SIGNS_ALL
        .iter()
        .map(|&(id, glyph, name)| Ref {
            id: format!("sign:{id}"),
            glyph: glyph.to_string(),
            name: name.to_string(),
        })
        .collect()
}

fn house_refs() -> Vec<HouseRef> {
    HOUSE_NAMES
        .iter()
        .enumerate()
        .map(|(i, &(label, name))| HouseRef {
            id: format!("house:{}", i + 1),
            label: label.to_string(),
            name: name.to_string(),
        })
        .collect()
}

fn birth_meta(input: &BirthInput, moment: &BirthMoment) -> Meta {
    Meta {
        name: input.name.clone(),
        born: format!(
            "{} {} ({}, UTC{})",
            input.date,
            input.time.format("%H:%M"),
            input.tz,
            moment.local.format("%:z")
        ),
        place: input.place.clone(),
        system: "Whole Sign".to_string(),
        zodiac: "Tropical".to_string(),
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
    fn historical_dst_offset_applied() {
        // Berlin, July 1990 → CEST (UTC+2): 14:30 local = 12:30 UT.
        let input = birth("1990-07-13", "14:30:00", 52.52, 13.405, "Europe/Berlin");
        let chart = compute_chart(&input).unwrap();
        assert!(chart.meta.born.contains("+02:00"), "born: {}", chart.meta.born);
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
