//! The one home for longitude arithmetic — the values every renderer used to
//! re-derive from a raw `lon`.
//!
//! Three renderers (the PDF in [`crate::pdf`], the self-contained artifact in
//! `templates/reading.html`, and the desktop's Svelte wheel) each carried their
//! own copy of "which sign is this longitude in", "how far into it", and "how
//! wide is this house" — four copies of some of them, in three languages, kept
//! in step by prose comments. The copies had already drifted: one normalized
//! before taking `% 30`, another did not.
//!
//! So the rule lives here, once, and the results ride on [`crate::contract`]
//! fields instead. Like `contract`, no pipeline stage owns this module: the
//! compute stage fills the fields for a fresh chart, and the desktop's
//! `load_chart` back-fills them for a chart saved before they existed. Both
//! call the same functions, which is the point — the inputs are
//! contract-shaped (a longitude, a pair of cusps), never geometry, because a
//! chart reloaded from disk may have no birth data to recompute from.
//!
//! Renderers keep what is genuinely theirs: their own radii, their own
//! glyph-crowding policy, and their own arc construction (krilla has no arc
//! primitive, so the PDF builds cubics where SVG writes an `A` command).

/// An ecliptic longitude in degrees, normalized to `[0, 360)` by construction.
///
/// The field is private so the invariant cannot be sidestepped: every way in
/// goes through [`Longitude::new`], which is the only `rem_euclid(360.0)` in
/// the codebase that callers need to know about. Before this type the
/// normalization was re-decided per call site — twice in Rust at two different
/// float widths, and twice again in JavaScript — and applied inconsistently.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct Longitude(f64);

impl Longitude {
    /// Normalize any degree value into `[0, 360)`. Accepts the out-of-range
    /// longitudes a hand-edited `chart.json` can carry: `contract::validate`
    /// deliberately does not reject those, because normalizing here makes them
    /// harmless rather than merely refused.
    pub fn new(deg: f64) -> Longitude {
        Longitude(deg.rem_euclid(360.0))
    }

    /// The normalized value, for the contract and for the renderers' trig.
    pub fn deg(self) -> f64 {
        self.0
    }

    /// The forward arc from `self` to `other`, in `[0, 360)` — counterclockwise
    /// along the ecliptic, the direction every renderer draws. This is the
    /// `norm360(l2 - l1)` that sector spans and house widths are both built on.
    pub fn arc_to(self, other: Longitude) -> f64 {
        (other.0 - self.0).rem_euclid(360.0)
    }
}

/// Where a longitude falls: which sign, and how far into it.
///
/// `sign` indexes [`crate::contract::ChartData::signs`], which
/// `contract::validate` guarantees is twelve long — so consumers index it
/// directly instead of carrying the `% 12` guard that the Rust copy had and
/// both JavaScript copies lacked.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Position {
    /// `0..=11`, zodiac order from 0° Aries.
    pub sign: u8,
    /// Whole degrees into the sign, `0..=29`.
    pub deg: u8,
    /// Arcminutes, `0..=59`.
    pub min: u8,
}

/// Resolve a longitude into sign, degree and minute.
///
/// The minute is *rounded*, so it can carry: at 29°59.6′ the minutes round to
/// 60, which advances the degree to 30 — and a 30th degree does not exist, so
/// the carry rolls on into the next sign. The renderers' copies of this
/// arithmetic carried into the degree but stopped there, and could print
/// `30°00′ Cancer`; here the same input reads `0°00′ Leo`.
pub fn position(lon: Longitude) -> Position {
    let mut sign = (lon.deg() / 30.0) as u8;
    let within = lon.deg() % 30.0;
    let mut deg = within.floor() as u8;
    let mut min = ((within - within.floor()) * 60.0).round() as u8;
    if min >= 60 {
        min = 0;
        deg += 1;
    }
    if deg >= 30 {
        deg = 0;
        sign += 1;
    }
    Position { sign: sign % 12, deg, min }
}

/// How wide a house is, from its own cusp to the next one.
///
/// Coincident cusps mean a full sign: quadrant systems can collapse two cusps
/// onto each other at extreme latitudes, and a zero-width wedge would vanish
/// from the wheel instead of spanning its sign. Every renderer already encoded
/// this — `norm360(next - c) || 30` in JavaScript, an `if` in Rust.
pub fn sweep(cusp: Longitude, next: Longitude) -> f64 {
    let arc = cusp.arc_to(next);
    if arc == 0.0 { 30.0 } else { arc }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longitude_normalizes_on_construction() {
        assert_eq!(Longitude::new(40.0).deg(), 40.0);
        assert_eq!(Longitude::new(400.0).deg(), 40.0);
        assert_eq!(Longitude::new(-30.0).deg(), 330.0);
        assert_eq!(Longitude::new(360.0).deg(), 0.0);
        assert_eq!(Longitude::new(-720.0).deg(), 0.0);
        // Every construction lands in range, including the values a
        // hand-edited chart.json could carry.
        for raw in [-1000.0, -0.5, 0.0, 359.999, 360.0, 1e4] {
            let d = Longitude::new(raw).deg();
            assert!((0.0..360.0).contains(&d), "{raw} normalized to {d}");
        }
    }

    #[test]
    fn arc_to_is_directional_and_wraps() {
        let a = Longitude::new(350.0);
        let b = Longitude::new(20.0);
        assert_eq!(a.arc_to(b), 30.0);
        assert_eq!(b.arc_to(a), 330.0);
        assert_eq!(a.arc_to(a), 0.0);
    }

    #[test]
    fn position_splits_into_sign_degree_minute() {
        // 0° Aries
        assert_eq!(position(Longitude::new(0.0)), Position { sign: 0, deg: 0, min: 0 });
        // 17°30' Cancer — Cancer is the 4th sign, index 3, spanning [90, 120)
        assert_eq!(position(Longitude::new(107.5)), Position { sign: 3, deg: 17, min: 30 });
        // last whole degree of Pisces
        assert_eq!(position(Longitude::new(359.0)), Position { sign: 11, deg: 29, min: 0 });
    }

    #[test]
    fn position_carries_minutes_into_the_next_sign() {
        // 29.9999° into Cancer: minutes round to 60, the degree would become 30
        // (which does not exist), so the carry advances the sign.
        let p = position(Longitude::new(119.9999));
        assert_eq!(p, Position { sign: 4, deg: 0, min: 0 }, "should roll into Leo");
        // The same carry at the end of the zodiac wraps back to Aries.
        let p = position(Longitude::new(359.9999));
        assert_eq!(p, Position { sign: 0, deg: 0, min: 0 }, "should roll into Aries");
        // A carry that only advances the degree stays inside its sign.
        let p = position(Longitude::new(100.99999));
        assert_eq!(p, Position { sign: 3, deg: 11, min: 0 });
    }

    #[test]
    fn position_is_always_in_range() {
        // Sweep the whole circle finely: no input may produce a sign outside
        // 0..=11, a degree outside 0..=29, or a minute outside 0..=59.
        let mut n = 0;
        for i in 0..36_000 {
            let p = position(Longitude::new(i as f64 / 100.0));
            assert!(p.sign < 12, "sign {} at {i}", p.sign);
            assert!(p.deg < 30, "deg {} at {i}", p.deg);
            assert!(p.min < 60, "min {} at {i}", p.min);
            n += 1;
        }
        assert_eq!(n, 36_000);
    }

    /// The arithmetic the three renderers carried, reproduced so the tests
    /// below can pin both where this module agrees with it and where it does not.
    fn legacy(lon: f64) -> (usize, f64, f64) {
        let within = lon.rem_euclid(360.0) % 30.0;
        let mut d = within.floor();
        let mut m = ((within - d) * 60.0).round();
        if m >= 60.0 {
            d += 1.0;
            m = 0.0;
        }
        let sign = (lon.rem_euclid(360.0) / 30.0) as usize % 12;
        (sign, d, m)
    }

    /// Ordinary positions must not shift: this is what makes the change safe
    /// to make in all three renderers at once.
    #[test]
    fn position_matches_the_legacy_formatter() {
        let mut compared = 0;
        for i in 0..36_000 {
            let lon = i as f64 / 100.0;
            let (ls, ld, lm) = legacy(lon);
            // A 0.01° grid never reaches the carry (see the test below), so
            // every sample here is an ordinary position.
            assert!(ld < 30.0, "grid unexpectedly hit the carry at {lon}");
            let p = position(Longitude::new(lon));
            assert_eq!(
                (p.sign as usize, p.deg as f64, p.min as f64),
                (ls, ld, lm),
                "drift at {lon}"
            );
            compared += 1;
        }
        assert_eq!(compared, 36_000);
    }

    /// The one place this module deliberately differs. Minutes are rounded, so
    /// a fractional degree at or above 59.5' carries into the degree — correct
    /// in itself, but the renderers' copies stopped there, so a carry in the
    /// *final* degree of a sign yielded a 30th degree that no sign has.
    ///
    /// The window is the last 0.5' of each of the twelve signs: 0.1° out of 360,
    /// or 0.028% of the circle. On an eleven-body chart that is roughly one
    /// chart in three hundred — rare, but a real rendering defect rather than a
    /// theoretical one, and it would print `30°00' Cancer`.
    #[test]
    fn corrects_the_legacy_thirtieth_degree() {
        // Every one of these lands inside a sign's final arcminute.
        let carries = [29.9999, 119.9999, 119.9958, 359.9999, 209.99306, 89.995];
        for lon in carries {
            let (ls, ld, lm) = legacy(lon);
            assert_eq!((ld, lm), (30.0, 0.0), "legacy should carry to 30° at {lon}");
            let p = position(Longitude::new(lon));
            assert_eq!(
                p,
                Position { sign: ((ls + 1) % 12) as u8, deg: 0, min: 0 },
                "should roll into the next sign at {lon}"
            );
        }

        // Size the window: the share of the circle the legacy copy mis-renders.
        // Expected is the last 0.5' of each of twelve signs — 0.1° of 360.
        let hits = (0..3_600_000)
            .filter(|i| legacy(*i as f64 / 10_000.0).1 >= 30.0)
            .count();
        let share = hits as f64 / 3_600_000.0;
        assert!(hits > 0, "the legacy carry never fired");
        assert!(
            (share - 0.1 / 360.0).abs() < 1e-4,
            "window is {:.4}% of the circle, expected {:.4}%",
            share * 100.0,
            0.1 / 360.0 * 100.0
        );
    }

    #[test]
    fn sweep_wraps_and_treats_coincident_cusps_as_a_full_sign() {
        assert_eq!(sweep(Longitude::new(0.0), Longitude::new(30.0)), 30.0);
        // the twelfth house closing the circle
        assert_eq!(sweep(Longitude::new(330.0), Longitude::new(0.0)), 30.0);
        // a collapsed quadrant cusp spans its sign rather than vanishing
        assert_eq!(sweep(Longitude::new(212.5), Longitude::new(212.5)), 30.0);
        // an intermediate quadrant width passes through untouched
        assert_eq!(sweep(Longitude::new(10.0), Longitude::new(48.0)), 38.0);
    }

    #[test]
    fn sweeps_over_a_full_cusp_ring_cover_the_circle() {
        // Whole Sign: twelve equal wedges summing to 360°.
        let cusps: Vec<Longitude> = (0..12).map(|i| Longitude::new(i as f64 * 30.0)).collect();
        let total: f64 = (0..12).map(|i| sweep(cusps[i], cusps[(i + 1) % 12])).sum();
        assert!((total - 360.0).abs() < 1e-9, "total {total}");

        // An uneven (quadrant-like) ring still tiles the circle exactly once.
        let raw = [12.0, 40.0, 71.0, 102.0, 140.0, 173.0, 192.0, 220.0, 251.0, 282.0, 320.0, 353.0];
        let cusps: Vec<Longitude> = raw.iter().map(|&d| Longitude::new(d)).collect();
        let total: f64 = (0..12).map(|i| sweep(cusps[i], cusps[(i + 1) % 12])).sum();
        assert!((total - 360.0).abs() < 1e-9, "total {total}");
    }
}
