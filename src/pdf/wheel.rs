//! The chart wheel as native PDF vector paths — a port of the artifact
//! viewer's SVG wheel (templates/reading.html), same radii and geometry,
//! recolored for the cream-paper rendition. Coordinates are y-down with the
//! origin at the page's top-left, exactly like the SVG, so the formulas
//! carry over unchanged.

use super::palette::*;
use super::{center_str, circle_path, fill, filled, stroke, stroked};
use crate::chart::separation;
use crate::contract::ChartData;
use crate::pdf::fonts::{Face, Fonts, glyph_face};
use krilla::geom::{Path, PathBuilder};
use krilla::surface::Surface;

/// Template radii (SVG units; outer label ring ≈ 361 units). Mirrored from
/// `templates/reading.html`'s `R = {...}` — change them there first.
const UNITS: f32 = 361.0;
const R_OUTER: f32 = 348.0;
const R_BAND_OUT: f32 = 344.0;
const R_SIGN_IN: f32 = 306.0;
const R_GRAD_IN: f32 = 294.0;
const R_PLANET: f32 = 260.0;
const R_WEDGE_OUT: f32 = 230.0;
const R_CHORD: f32 = 222.0;
const R_HUB: f32 = 92.0;
const R_HOUSE_LBL: f32 = 112.0;

struct Plate {
    cx: f32,
    cy: f32,
    /// Points per template unit.
    k: f32,
    asc: f32,
}

impl Plate {
    /// ASC on the left; ecliptic longitude increases counterclockwise.
    fn angle(&self, lon: f32) -> f32 {
        std::f32::consts::PI + (lon - self.asc).to_radians()
    }

    fn pt(&self, lon: f32, r: f32) -> (f32, f32) {
        let a = self.angle(lon);
        (self.cx + self.k * r * a.cos(), self.cy - self.k * r * a.sin())
    }

    /// Circle point at raw angle `t` (radians, y-flipped like `pt`).
    fn at(&self, t: f32, r: f32) -> (f32, f32) {
        (self.cx + self.k * r * t.cos(), self.cy - self.k * r * t.sin())
    }

    /// Append a circular arc from angle `t0` to `t1` (radians) at radius `r`
    /// as cubic segments; the path must already be at the arc's start.
    fn arc(&self, pb: &mut PathBuilder, r: f32, t0: f32, t1: f32) {
        let rr = self.k * r;
        let n = ((t1 - t0).abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
        let step = (t1 - t0) / n as f32;
        let h = 4.0 / 3.0 * (step / 4.0).tan();
        for i in 0..n {
            let a0 = t0 + step * i as f32;
            let a1 = a0 + step;
            let (x0, y0) = self.at(a0, r);
            let (x3, y3) = self.at(a1, r);
            // derivative of (cos, -sin), scaled by radius
            let (c1x, c1y) = (x0 - h * rr * a0.sin(), y0 - h * rr * a0.cos());
            let (c2x, c2y) = (x3 + h * rr * a1.sin(), y3 + h * rr * a1.cos());
            pb.cubic_to(c1x, c1y, c2x, c2y, x3, y3);
        }
    }

    fn circle(&self, r: f32) -> Path {
        circle_path(self.cx, self.cy, self.k * r)
    }

    /// Annular sector between longitudes `l1`→`l2` (ccw), radii `r1 < r2` —
    /// the template's `sector()`.
    fn sector(&self, l1: f32, l2: f32, r1: f32, r2: f32) -> Path {
        let (t1, t2) = (self.angle(l1), self.angle(l2));
        let mut pb = PathBuilder::new();
        let (x, y) = self.at(t1, r2);
        pb.move_to(x, y);
        self.arc(&mut pb, r2, t1, t2);
        let (x, y) = self.at(t2, r1);
        pb.line_to(x, y);
        self.arc(&mut pb, r1, t2, t1);
        pb.close();
        pb.finish().expect("sector path")
    }

    /// Append a radial segment to `pb`.
    fn seg(&self, pb: &mut PathBuilder, lon: f32, r1: f32, r2: f32) {
        let (x1, y1) = self.pt(lon, r1);
        let (x2, y2) = self.pt(lon, r2);
        pb.move_to(x1, y1);
        pb.line_to(x2, y2);
    }

    fn line(&self, lon: f32, r1: f32, r2: f32) -> Path {
        let mut pb = PathBuilder::new();
        self.seg(&mut pb, lon, r1, r2);
        pb.finish().expect("line path")
    }
}

fn norm360(x: f32) -> f32 {
    x.rem_euclid(360.0)
}

/// Centered text at a wheel position (the SVG's text-anchor middle +
/// dominant-baseline central).
#[allow(clippy::too_many_arguments)]
fn label(
    s: &mut Surface,
    fonts: &Fonts,
    face: Face,
    size: f32,
    color: (u8, u8, u8),
    x: f32,
    y: f32,
    text: &str,
) {
    center_str(s, fonts, face, size, color, x, y + size * 0.34, text);
}

/// Draw the full plate with its center at (`cx`, `cy`) and the outermost
/// label ring at `radius` points.
pub fn draw(s: &mut Surface, fonts: &Fonts, chart: &ChartData, cx: f32, cy: f32, radius: f32) {
    let p = Plate { cx, cy, k: radius / UNITS, asc: chart.axes.asc as f32 };
    let k = p.k;

    // engraved concentric rings
    for (r, strong) in [
        (R_OUTER, true),
        (R_BAND_OUT, true),
        (R_SIGN_IN, false),
        (R_GRAD_IN, false),
        (R_WEDGE_OUT, false),
        (R_HUB, false),
        (R_HUB - 4.0, false),
    ] {
        stroked(s, &p.circle(r), stroke(if strong { HAIRLINE } else { LINE }, 0.9 * k.max(0.7), 1.0));
    }

    // graduation band: 1° / 5° / 10° ticks, one path per tick class
    let mut grads = [PathBuilder::new(), PathBuilder::new(), PathBuilder::new()];
    for d in 0..360 {
        let (class, len) = match d {
            d if d % 10 == 0 => (0, 12.0),
            d if d % 5 == 0 => (1, 8.0),
            _ => (2, 4.5),
        };
        p.seg(&mut grads[class], d as f32, R_SIGN_IN - len, R_SIGN_IN);
    }
    for (pb, w) in grads.into_iter().zip([0.9, 0.7, 0.45]) {
        stroked(s, &pb.finish().expect("ticks"), stroke(LINE, (w * k).max(0.3), 1.0));
    }

    // centre ornament: compass rays, one path
    let mut rays = PathBuilder::new();
    for i in 0..8 {
        let len = if i % 2 == 0 { 22.0 } else { 13.0 };
        p.seg(&mut rays, i as f32 * 45.0, 5.0, len);
    }
    stroked(s, &rays.finish().expect("rays"), stroke(LINE, 0.8 * k, 1.0));
    stroked(s, &p.circle(3.0), stroke(HAIRLINE, 0.9 * k, 1.0));

    // sign band: element washes under verdigris identity
    for (i, sign) in chart.signs.iter().enumerate() {
        let lon = i as f32 * 30.0;
        let path = p.sector(lon, lon + 30.0, R_SIGN_IN, R_BAND_OUT);
        filled(s, &path, fill(wash(&sign.element), 1.0));
        stroked(s, &path, stroke(LINE, 0.7 * k, 1.0));
        let (gx, gy) = p.pt(lon + 15.0, 325.0);
        label(s, fonts, Face::Symbols, 21.0 * k, VERDIGRIS, gx, gy, &sign.glyph);
    }

    // house cusp spokes + roman labels
    let cusps: Vec<f32> = chart.house_cusps.iter().map(|c| *c as f32).collect();
    let mut spokes = PathBuilder::new();
    for (i, house) in chart.houses.iter().enumerate() {
        let c = cusps[i];
        let next = cusps[(i + 1) % 12];
        p.seg(&mut spokes, c, R_HUB, R_GRAD_IN);
        let sweep = if norm360(next - c) == 0.0 { 30.0 } else { norm360(next - c) };
        let (lx, ly) = p.pt(c + sweep / 2.0, R_HOUSE_LBL);
        label(s, fonts, Face::Regular, 11.0 * k, STEEL, lx, ly, &house.label);
    }
    stroked(s, &spokes.finish().expect("spokes"), stroke(LINE, 0.7 * k, 1.0));

    // ASC/MC axes
    let axes = [
        (chart.axes.asc as f32, "AC"),
        (chart.axes.mc as f32, "MC"),
        (chart.axes.asc as f32 + 180.0, "DC"),
        (chart.axes.mc as f32 + 180.0, "IC"),
    ];
    for (lon, name) in axes {
        stroked(s, &p.line(lon, R_HUB, R_OUTER), stroke(HAIRLINE, 1.4 * k, 1.0));
        let (tx, ty) = p.pt(lon, R_OUTER + 13.0);
        label(s, fonts, Face::Regular, 10.0 * k, INK3, tx, ty, name);
    }

    // aspect chords, under the planet glyphs — colored by NATURE (classic
    // blue/red), not the category color, mirroring the artifact wheel
    for a in &chart.aspects {
        if let (Some(pa), Some(pb)) = (chart.planet(&a.a), chart.planet(&a.b)) {
            let color = match a.nature.as_str() {
                "harmonious" => STEEL,
                "challenging" => OXBLOOD,
                _ => INK3, // conjunction: neutral blending
            };
            let mut pb2 = PathBuilder::new();
            let (x1, y1) = p.pt(pa.lon as f32, R_CHORD);
            let (x2, y2) = p.pt(pb.lon as f32, R_CHORD);
            pb2.move_to(x1, y1);
            pb2.line_to(x2, y2);
            stroked(s, &pb2.finish().expect("chord"), stroke(color, 1.2 * k, 0.6));
        }
    }

    // planets + Ascendant point, nudged inward on crowding
    let mut by_lon: Vec<_> = chart.planets.iter().collect();
    by_lon.sort_by(|a, b| a.lon.total_cmp(&b.lon));
    let mut prev: Option<(f64, f32)> = None; // (lon, radius)
    for body in by_lon {
        let r = match prev {
            Some((plon, pr)) if separation(body.lon, plon) < 8.0 => (pr - 27.0).max(176.0),
            _ => R_PLANET,
        };
        prev = Some((body.lon, r));
        let lon = body.lon as f32;

        stroked(s, &p.line(lon, R_GRAD_IN - 8.0, R_GRAD_IN), stroke(BRASS, 1.1 * k, 1.0));
        let (gx, gy) = p.pt(lon, r);
        let face = glyph_face(&body.glyph);
        let size = if face == Face::Regular { 13.0 } else { 22.0 };
        label(s, fonts, face, size * k, BRASS, gx, gy, &body.glyph);
        let (dx, dy) = p.pt(lon, r - 21.0);
        let deg = format!("{}\u{b0}", (norm360(lon) % 30.0).floor());
        label(s, fonts, Face::Regular, 8.5 * k, INK3, dx, dy, &deg);
    }
}
