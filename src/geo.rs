//! Offline gazetteer: city query → latitude, longitude, and IANA timezone.
//!
//! Data is GeoNames `cities500` (CC-BY 4.0), stripped and embedded at build
//! time by `build.rs`. Every place row carries its IANA timezone id, so one
//! lookup yields all three birth-chart inputs; historical UTC offsets then
//! come from chrono-tz as before.
//!
//! `search` is pure and synchronous — the future TUI typeahead calls it per
//! keystroke; the CLI calls `resolve` on top of it.

use chrono_tz::Tz;
use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

static PLACES_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/places.tsv.gz"));

/// One gazetteer row. Owned strings are kept to a minimum: `admin1`,
/// `country`, and `cc` repeat across thousands of rows and are interned by the
/// private `Interner`; the lowercase display name is stored only for the
/// minority of names where it differs from the folded ascii form.
pub struct Place {
    pub id: u32,
    pub name: String,
    pub admin1: &'static str,
    pub country: &'static str,
    pub cc: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub pop: u64,
    pub tz: Tz,
    /// Lowercased asciiname — the primary match key.
    ascii_lower: String,
    /// Lowercased `name`, only when it differs from `ascii_lower`
    /// (e.g. "münchen" vs "munchen").
    name_lower: Option<String>,
    admin1_lower: &'static str,
    country_lower: &'static str,
}

impl Place {
    pub fn label(&self) -> String {
        if self.admin1.is_empty() || self.admin1 == self.name {
            format!("{}, {}", self.name, self.country)
        } else {
            format!("{}, {}, {}", self.name, self.admin1, self.country)
        }
    }

    fn matches_qualifier(&self, q: &str) -> bool {
        self.cc.eq_ignore_ascii_case(q)
            || self.country_lower == q
            || self.admin1_lower.starts_with(q)
    }

    /// The exact-name predicate shared by `search`'s top tier and `resolve`.
    fn name_is(&self, city: &str) -> bool {
        self.ascii_lower == city || self.name_lower.as_deref() == Some(city)
    }

    fn name_starts_with(&self, city: &str) -> bool {
        self.ascii_lower.starts_with(city)
            || self.name_lower.as_deref().is_some_and(|n| n.starts_with(city))
    }

    fn name_contains(&self, city: &str) -> bool {
        self.ascii_lower.contains(city)
            || self.name_lower.as_deref().is_some_and(|n| n.contains(city))
    }
}

/// Deduplicates the handful of distinct admin/country strings across ~235k
/// rows, handing out `&'static str`s. Leaking is fine: the gazetteer itself
/// lives for the process (`OnceLock`), and distinct values number in the
/// low thousands.
#[derive(Default)]
struct Interner(HashMap<String, &'static str>);

impl Interner {
    fn get(&mut self, s: &str) -> &'static str {
        if let Some(&v) = self.0.get(s) {
            return v;
        }
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
        self.0.insert(s.to_string(), leaked);
        leaked
    }
}

fn raw_tsv() -> String {
    let mut s = String::new();
    flate2::read::GzDecoder::new(PLACES_GZ)
        .read_to_string(&mut s)
        .expect("embedded gazetteer is corrupt");
    s
}

/// All places, population-descending (the order build.rs wrote them).
/// Rows whose timezone chrono-tz cannot parse are skipped defensively;
/// the `every_timezone_parses` test keeps that set empty in practice.
fn places() -> &'static [Place] {
    static PLACES: OnceLock<Vec<Place>> = OnceLock::new();
    PLACES.get_or_init(|| {
        let mut intern = Interner::default();
        raw_tsv()
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() != 10 {
                    return None;
                }
                let name = f[1].to_string();
                let ascii_lower = f[2].to_lowercase();
                let name_lower = Some(name.to_lowercase()).filter(|n| *n != ascii_lower);
                Some(Place {
                    id: f[0].parse().ok()?,
                    ascii_lower,
                    name_lower,
                    name,
                    lat: f[3].parse().ok()?,
                    lon: f[4].parse().ok()?,
                    admin1_lower: intern.get(&f[5].to_lowercase()),
                    admin1: intern.get(f[5]),
                    country_lower: intern.get(&f[6].to_lowercase()),
                    country: intern.get(f[6]),
                    cc: intern.get(f[7]),
                    pop: f[8].parse().ok()?,
                    tz: f[9].parse().ok()?,
                })
            })
            .collect()
    })
}

/// Row positions by GeoNames id, sorted for binary search.
///
/// The rows themselves are population-descending, which is the order every
/// *search* wants and the worst possible order for an id lookup: `by_id` was a
/// linear scan, so its cost fell on whoever's birthplace is small enough to sit
/// near the end of 235k rows. It is not a rare path — `preview` resolves the
/// place on every round of a live scrub, for a place id that cannot change
/// during one, and a full scan measured about as expensive as the ephemeris it
/// precedes (376µs against 387µs).
///
/// Two `u32` per row is ~1.9 MB beside a gazetteer already tens of megabytes
/// wide, and it is built from `places()` so the two can never disagree.
fn by_id_index() -> &'static [(u32, u32)] {
    static INDEX: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index: Vec<(u32, u32)> =
            places().iter().enumerate().map(|(row, p)| (p.id, row as u32)).collect();
        index.sort_unstable_by_key(|(id, _)| *id);
        index
    })
}

/// Force the one-time decompress/parse of the embedded gazetteer now, so an
/// interactive caller can front-load it instead of paying on the first search.
pub fn warm() {
    let _ = places();
    let _ = by_id_index();
}

/// Split "city, qualifier, qualifier" into a lowercase city token + qualifiers.
fn parse_query(query: &str) -> Option<(String, Vec<String>)> {
    let mut parts = query
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let city = parts.next()?;
    Some((city, parts.collect()))
}

/// Ranked search: exact name > prefix > substring, each tier population-
/// descending (rows are pre-sorted, so tier filtering preserves that order).
pub fn search(query: &str, limit: usize) -> Vec<&'static Place> {
    let Some((city, quals)) = parse_query(query) else {
        return Vec::new();
    };
    let mut exact = Vec::new();
    let mut prefix = Vec::new();
    let mut substr = Vec::new();
    // Rows are population-sorted, so each tier fills in final order and the
    // scan can stop once the exact tier alone satisfies the limit; the
    // cheaper capacity checks run before the string scans.
    for p in places() {
        if !quals.iter().all(|q| p.matches_qualifier(q)) {
            continue;
        }
        if p.name_is(&city) {
            exact.push(p);
            if exact.len() >= limit {
                break;
            }
        } else if prefix.len() < limit && p.name_starts_with(&city) {
            prefix.push(p);
        } else if substr.len() < limit && p.name_contains(&city) {
            substr.push(p);
        }
    }
    exact.into_iter().chain(prefix).chain(substr).take(limit).collect()
}

pub enum Resolution {
    Match(&'static Place),
    Ambiguous(Vec<&'static Place>),
    NotFound,
}

/// Resolve a query to a single place when it is safe to do so:
/// one exact-name match, or a dominant one (≥10× the runner-up's population —
/// "berlin" is Berlin, DE; "springfield" is a list).
pub fn resolve(query: &str) -> Resolution {
    let Some((city, _)) = parse_query(query) else {
        return Resolution::NotFound;
    };
    let candidates = search(query, 8);
    if candidates.is_empty() {
        return Resolution::NotFound;
    }
    let exact: Vec<&&Place> = candidates.iter().filter(|p| p.name_is(&city)).collect();
    match exact.len() {
        0 => Resolution::Ambiguous(candidates),
        1 => Resolution::Match(exact[0]),
        _ if exact[0].pop >= 10 * exact[1].pop.max(1) => Resolution::Match(exact[0]),
        _ => Resolution::Ambiguous(candidates),
    }
}

/// The place with this GeoNames id. The id is the stable handle every frontend
/// round-trips a place by, so this is the hot lookup, not the search.
pub fn by_id(id: u32) -> Option<&'static Place> {
    let index = by_id_index();
    let at = index.binary_search_by_key(&id, |(id, _)| *id).ok()?;
    places().get(index[at].1 as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn berlin_resolves_by_population_dominance() {
        match resolve("berlin") {
            Resolution::Match(p) => {
                assert_eq!(p.cc, "DE");
                assert_eq!(p.tz, chrono_tz::Europe::Berlin);
                assert!((p.lat - 52.52).abs() < 0.1, "lat {}", p.lat);
            }
            _ => panic!("berlin should auto-resolve to Berlin, DE"),
        }
    }

    #[test]
    fn qualifiers_disambiguate_portland() {
        let (or, me) = (resolve("portland, oregon"), resolve("portland, maine"));
        match (or, me) {
            (Resolution::Match(a), Resolution::Match(b)) => {
                assert_eq!(a.tz, chrono_tz::America::Los_Angeles);
                assert_eq!(b.tz, chrono_tz::America::New_York);
            }
            _ => panic!("qualified portland queries should each resolve"),
        }
    }

    #[test]
    fn springfield_is_ambiguous() {
        assert!(matches!(resolve("springfield"), Resolution::Ambiguous(_)));
    }

    #[test]
    fn exact_match_outranks_prefix() {
        let hits = search("paris", 5);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].cc, "FR", "expected Paris, FR first, got {}", hits[0].label());
    }

    #[test]
    fn unknown_place_is_not_found() {
        assert!(matches!(resolve("xqzzyplugh"), Resolution::NotFound));
    }

    /// The index must answer exactly what a scan over the rows would, or a
    /// place id round-tripped from a saved chart resolves to the wrong city.
    #[test]
    fn by_id_agrees_with_a_scan_over_every_row() {
        let rows = places();
        for p in rows {
            let found = by_id(p.id).expect("every row is findable by its own id");
            assert_eq!(found.id, p.id);
            // Same row, not merely an equal id.
            assert!(std::ptr::eq(found, p), "id {} resolved to a different row", p.id);
        }
        assert!(rows.len() > 200_000, "suspiciously small gazetteer: {}", rows.len());
    }

    /// Binary search needs one row per id; a duplicate would make `by_id`
    /// return an arbitrary one of them.
    #[test]
    fn geonames_ids_are_unique_across_the_dataset() {
        let index = by_id_index();
        assert_eq!(index.len(), places().len());
        let duplicate = index.windows(2).find(|w| w[0].0 == w[1].0);
        assert!(duplicate.is_none(), "duplicate geonames id: {duplicate:?}");
    }

    #[test]
    fn an_absent_id_is_none() {
        assert!(by_id(u32::MAX).is_none());
        assert!(by_id(0).is_none());
    }

    /// The lookup must not depend on how populous the place is — that was the
    /// whole defect. A village and a capital resolve in the same number of
    /// comparisons, so compare against the row a scan finds rather than timing.
    #[test]
    fn a_place_at_the_far_end_of_the_ordering_resolves_like_one_at_the_front() {
        let rows = places();
        let first = &rows[0];
        let last = &rows[rows.len() - 1];
        assert!(last.pop <= first.pop, "rows should be population-descending");
        assert!(std::ptr::eq(by_id(first.id).unwrap(), first));
        assert!(std::ptr::eq(by_id(last.id).unwrap(), last));
    }

    /// Every distinct timezone string in the embedded dataset must parse as a
    /// chrono-tz timezone — catches GeoNames↔chrono-tz drift when the
    /// gazetteer is regenerated.
    #[test]
    fn every_timezone_parses() {
        let raw = raw_tsv();
        let tzs: BTreeSet<&str> = raw
            .lines()
            .filter_map(|l| l.split('\t').nth(9))
            .filter(|s| !s.is_empty())
            .collect();
        assert!(tzs.len() > 300, "suspiciously few timezones: {}", tzs.len());
        let bad: Vec<&&str> = tzs.iter().filter(|t| t.parse::<Tz>().is_err()).collect();
        assert!(bad.is_empty(), "unparsable timezones: {bad:?}");
    }

    #[test]
    fn dataset_is_population_sorted_and_large() {
        let p = places();
        assert!(p.len() > 100_000, "only {} places embedded", p.len());
        assert!(p[0].pop >= p[p.len() - 1].pop);
    }
}
