//! The engraved plate, specified once — the geometry all three renditions of
//! the wheel are drawings of.
//!
//! Like [`crate::contract`] and [`crate::derive`], no stage owns this: the PDF
//! reads it directly, [`crate::emit`] substitutes it into the artifact beside
//! the chart, and the desktop reads the generated
//! `desktop/src/lib/generated/plate.ts`.
//!
//! It exists because the three carried their own copies. `src/pdf/wheel.rs`
//! said "mirrored from the template — change them there first", the template
//! said "canonical — mirrored by the Svelte wheel and the PDF", and the Svelte
//! wheel said it ports the template. Nothing checked any of it, and five values
//! had drifted apart by the time anyone looked: the house-label radius, the
//! decade tick length, the ring count, the cusp-spoke inner radius, and the
//! planet de-crowding rule.
//!
//! **What belongs here** is the plate's identity: where its rings sit, how its
//! graduations are classed, when bodies stop overlapping. **What does not** is
//! how a given medium draws it — the PDF builds cubic arcs because krilla has
//! no arc primitive, SVG writes an `A` command, and type sizes are points on
//! paper against CSS pixels on screen. Those stay with their renderer, as
//! CONTEXT.md's counterpart rule says they should.
//!
//! A rendition may still depart from the spec. The desktop orrery is
//! deliberately richer than paper — it has a core medallion the others lack, so
//! its house labels sit further out. The point is that a departure is now an
//! override written next to the reason, and everything not overridden cannot
//! drift.

use serde::Serialize;

/// Ring radii, in plate units (the artifact's SVG user units — the PDF scales
/// them by its own plate size).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Radii {
    /// The outermost engraved ring; the axis labels sit just beyond it.
    pub outer: f32,
    /// Outer edge of the sign band.
    pub band_out: f32,
    /// Inner edge of the sign band, where the graduations end.
    pub sign_in: f32,
    /// Inner edge of the graduation band.
    pub grad_in: f32,
    /// Where an uncrowded body's glyph sits.
    pub planet: f32,
    /// Outer edge of the house-wedge band.
    pub wedge_out: f32,
    /// The aspect chords' circle.
    pub chord: f32,
    /// The hub ring — the inner edge of the house band.
    pub hub: f32,
    /// Where a house's roman numeral sits.
    pub house_label: f32,
    /// Where a sign's glyph sits, at the midpoint of its band.
    pub sign_glyph: f32,
}

/// The 1° / 5° / 10° graduation hierarchy: how long and how heavy each class
/// of tick is.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticks {
    pub decade_len: f32,
    pub five_len: f32,
    pub unit_len: f32,
    pub decade_width: f32,
    pub five_width: f32,
    pub unit_width: f32,
}

/// When two bodies are too close to read side by side, the later one steps
/// inward. Without this a stellium prints as one illegible pile.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crowding {
    /// Bodies closer than this many degrees are crowded.
    pub threshold_deg: f32,
    /// How far inward a crowded body steps from the one before it.
    pub step: f32,
    /// It never steps closer to the hub than this.
    pub floor: f32,
}

/// The whole specification.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plate {
    pub radii: Radii,
    pub ticks: Ticks,
    pub crowding: Crowding,
    /// Gap from [`Radii::outer`] out to the AC/MC/DC/IC labels. Also what makes
    /// the plate's overall extent — see [`Plate::units`].
    pub axis_label_gap: f32,
    /// How far inside [`Radii::grad_in`] a body's tick reaches.
    pub body_tick_len: f32,
    /// How far inside a body's glyph its degree label sits.
    pub degree_label_drop: f32,
}

/// The plate every rendition draws.
pub const PLATE: Plate = Plate {
    radii: Radii {
        outer: 348.0,
        band_out: 344.0,
        sign_in: 306.0,
        grad_in: 294.0,
        planet: 260.0,
        wedge_out: 230.0,
        chord: 222.0,
        hub: 92.0,
        house_label: 112.0,
        sign_glyph: 325.0,
    },
    ticks: Ticks {
        decade_len: 12.0,
        five_len: 8.0,
        unit_len: 4.5,
        decade_width: 0.9,
        five_width: 0.7,
        unit_width: 0.45,
    },
    crowding: Crowding { threshold_deg: 8.0, step: 27.0, floor: 176.0 },
    axis_label_gap: 13.0,
    body_tick_len: 8.0,
    degree_label_drop: 21.0,
};

impl Plate {
    /// The plate's full extent, out to the axis labels — the divisor a fixed-size
    /// rendition scales by. The PDF carried this as a hand-derived `361.0`, so
    /// moving the axis labels silently rescaled the whole page.
    pub const fn units(&self) -> f32 {
        self.radii.outer + self.axis_label_gap
    }

    /// The concentric rings, outermost first, each with whether it is engraved
    /// heavily. The two outer rings carry the plate's edge; the rest are
    /// hairlines.
    pub fn rings(&self) -> [(f32, bool); 7] {
        let r = &self.radii;
        [
            (r.outer, true),
            (r.band_out, true),
            (r.sign_in, false),
            (r.grad_in, false),
            (r.wedge_out, false),
            (r.hub, false),
            (r.hub - 4.0, false),
        ]
    }

    /// The length and weight of the graduation at whole degree `d`.
    pub fn tick(&self, d: u32) -> (f32, f32) {
        let t = &self.ticks;
        match d {
            _ if d.is_multiple_of(10) => (t.decade_len, t.decade_width),
            _ if d.is_multiple_of(5) => (t.five_len, t.five_width),
            _ => (t.unit_len, t.unit_width),
        }
    }

    /// Where a body's glyph sits, given the one before it going round.
    ///
    /// `previous` is the last body placed and the radius it took; bodies are
    /// walked in longitude order, so crowding cascades — three bodies in a
    /// degree step inward one after another.
    pub fn crowded_radius(&self, lon: f64, previous: Option<(f64, f32)>) -> f32 {
        match previous {
            Some((prev_lon, prev_r))
                if crate::chart::separation(lon, prev_lon) < self.crowding.threshold_deg as f64 =>
            {
                (prev_r - self.crowding.step).max(self.crowding.floor)
            }
            _ => self.radii.planet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rings_descend_from_the_edge_to_the_hub() {
        let rings = PLATE.rings();
        for pair in rings.windows(2) {
            assert!(pair[0].0 > pair[1].0, "rings must be ordered outward-in: {pair:?}");
        }
        assert!(rings.iter().filter(|(_, strong)| *strong).count() == 2, "two heavy rings");
    }

    /// Every radius has to sit inside the plate and outside the centre, or a
    /// band renders inverted.
    #[test]
    fn every_radius_lies_within_the_plate() {
        let r = &PLATE.radii;
        for (name, v) in [
            ("outer", r.outer), ("bandOut", r.band_out), ("signIn", r.sign_in),
            ("gradIn", r.grad_in), ("planet", r.planet), ("wedgeOut", r.wedge_out),
            ("chord", r.chord), ("hub", r.hub), ("houseLabel", r.house_label),
            ("signGlyph", r.sign_glyph),
        ] {
            assert!(v > 0.0 && v <= PLATE.units(), "{name} = {v} is outside the plate");
        }
        assert!(r.band_out < r.outer, "the sign band sits inside the edge");
        assert!(r.sign_in < r.band_out);
        assert!(r.sign_glyph > r.sign_in && r.sign_glyph < r.band_out, "sign glyphs sit in their band");
        assert!(r.grad_in < r.sign_in, "graduations run inward from the sign band");
        assert!(r.planet < r.grad_in, "bodies sit inside the graduations");
        assert!(r.wedge_out < r.planet);
        assert!(r.chord < r.wedge_out);
        assert!(r.hub < r.chord);
    }

    #[test]
    fn ticks_are_a_hierarchy() {
        let t = &PLATE.ticks;
        assert!(t.decade_len > t.five_len && t.five_len > t.unit_len);
        assert!(t.decade_width > t.five_width && t.five_width > t.unit_width);
        assert_eq!(PLATE.tick(0), (t.decade_len, t.decade_width), "0° is a decade");
        assert_eq!(PLATE.tick(30), (t.decade_len, t.decade_width));
        assert_eq!(PLATE.tick(25), (t.five_len, t.five_width));
        assert_eq!(PLATE.tick(7), (t.unit_len, t.unit_width));
        // The band between `gradIn` and `signIn` is exactly what a decade tick
        // spans — that is what makes the band read as a scale rather than a
        // gap. Nothing may reach past it.
        let band = PLATE.radii.sign_in - PLATE.radii.grad_in;
        assert_eq!(t.decade_len, band, "the decade tick defines the graduation band");
        assert!(t.five_len < band && t.unit_len < band);
    }

    #[test]
    fn units_reach_the_axis_labels() {
        assert_eq!(PLATE.units(), 361.0, "the PDF's hand-derived divisor");
        assert!(PLATE.units() > PLATE.radii.outer);
    }

    #[test]
    fn an_uncrowded_body_sits_on_the_planet_ring() {
        assert_eq!(PLATE.crowded_radius(100.0, None), PLATE.radii.planet);
        // Far from the one before it.
        assert_eq!(PLATE.crowded_radius(100.0, Some((40.0, 260.0))), PLATE.radii.planet);
    }

    #[test]
    fn a_crowded_body_steps_inward_from_the_one_before_it() {
        let c = &PLATE.crowding;
        let r = PLATE.crowded_radius(100.0, Some((97.0, PLATE.radii.planet)));
        assert_eq!(r, PLATE.radii.planet - c.step);
        // Exactly at the threshold is not crowded — the comparison is strict.
        assert_eq!(
            PLATE.crowded_radius(100.0, Some((100.0 - c.threshold_deg as f64, 260.0))),
            PLATE.radii.planet
        );
    }

    #[test]
    fn crowding_wraps_across_zero_degrees() {
        // 359° and 2° are three degrees apart, not 357.
        let r = PLATE.crowded_radius(2.0, Some((359.0, PLATE.radii.planet)));
        assert_eq!(r, PLATE.radii.planet - PLATE.crowding.step);
    }

    #[test]
    fn a_stellium_stacks_inward_but_never_past_the_floor() {
        // Twenty bodies within a degree of each other: each steps in from the
        // last, and the pile stops rather than crossing the hub.
        let mut prev: Option<(f64, f32)> = None;
        let mut radii = Vec::new();
        for i in 0..20 {
            let lon = 100.0 + i as f64 * 0.1;
            let r = PLATE.crowded_radius(lon, prev);
            radii.push(r);
            prev = Some((lon, r));
        }
        assert!(radii.windows(2).all(|w| w[1] <= w[0]), "each steps inward or holds");
        assert!(
            radii.iter().all(|r| *r >= PLATE.crowding.floor),
            "never past the floor: {radii:?}"
        );
        assert!(*radii.last().unwrap() > PLATE.radii.hub, "and never into the hub");
    }
}

/// Generate the webview's copy of the specification, alongside the ts-rs
/// bindings.
///
/// A test rather than a build script for the same reason the command names are:
/// `npm run gen:types` and CI both run `cargo test ... export_bindings` and diff
/// the committed output, so this rides the drift check that already exists.
#[cfg(all(test, feature = "ts"))]
#[test]
fn export_bindings_plate() {
    let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else {
        return;
    };
    let json = serde_json::to_string_pretty(&PLATE).expect("the plate serializes");
    let out = format!(
        "// This file was generated from `src/plate.rs`. Do not edit this file manually.\n\n\
         /** The engraved plate every rendition of the wheel draws — the radii, the\n\
         \x20* graduation classes and the de-crowding rule, stated once in Rust and read\n\
         \x20* here, by the PDF, and by the emitted artifact.\n\
         \x20*\n\
         \x20* A rendition may depart from it, but a departure should be an override\n\
         \x20* written next to its reason — see `Wheel.svelte`. */\n\
         export const PLATE = {json};\n\n\
         export type Plate = typeof PLATE;\n"
    );
    let path = std::path::Path::new(&dir).join("generated").join("plate.ts");
    std::fs::create_dir_all(path.parent().expect("generated dir")).expect("create generated dir");
    std::fs::write(&path, out).expect("write plate.ts");
}
