//! Paint and low-level path helpers over krilla — the drawing primitives the
//! wheel, the flow engine, and the page driver all build on. No astrology or
//! layout knowledge lives here; everything takes explicit colors and points.

use krilla::geom::{Path as KPath, PathBuilder, Rect};
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;

pub(crate) fn solid(c: (u8, u8, u8)) -> krilla::paint::Paint {
    krilla::color::rgb::Color::new(c.0, c.1, c.2).into()
}

pub(crate) fn fill(c: (u8, u8, u8), alpha: f32) -> Fill {
    Fill {
        paint: solid(c),
        opacity: NormalizedF32::new(alpha).unwrap_or(NormalizedF32::ONE),
        ..Default::default()
    }
}

pub(crate) fn stroke(c: (u8, u8, u8), width: f32, alpha: f32) -> Stroke {
    Stroke {
        paint: solid(c),
        width,
        opacity: NormalizedF32::new(alpha).unwrap_or(NormalizedF32::ONE),
        ..Default::default()
    }
}

/// Fill and stroke are persistent surface state in krilla — every paint
/// helper must clear the one it doesn't use, or paths inherit stale paint.
pub(crate) fn stroked(s: &mut Surface, path: &KPath, st: Stroke) {
    s.set_fill(None);
    s.set_stroke(Some(st));
    s.draw_path(path);
}

pub(crate) fn filled(s: &mut Surface, path: &KPath, f: Fill) {
    s.set_stroke(None);
    s.set_fill(Some(f));
    s.draw_path(path);
}

/// A circle as four cubic segments — krilla's `PathBuilder` has no circle
/// primitive. The one bezier-circle in the module (the wheel's rings and the
/// compass both build on it).
pub(crate) fn circle_path(cx: f32, cy: f32, r: f32) -> KPath {
    const KAPPA: f32 = 0.552_284_8; // 4/3·tan(π/8)
    let k = KAPPA * r;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    pb.close();
    pb.finish().expect("circle path")
}

pub(crate) fn hline(s: &mut Surface, x1: f32, x2: f32, y: f32, color: (u8, u8, u8), width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y);
    pb.line_to(x2, y);
    stroked(s, &pb.finish().expect("hline"), stroke(color, width, 1.0));
}

pub(crate) fn rect_stroke(
    s: &mut Surface,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: (u8, u8, u8),
    width: f32,
) {
    let mut pb = PathBuilder::new();
    pb.push_rect(Rect::from_xywh(x, y, w, h).expect("rect"));
    stroked(s, &pb.finish().expect("rect path"), stroke(color, width, 1.0));
}
