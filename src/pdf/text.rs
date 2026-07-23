//! The typography layer: drawing, letterspacing, centering, and greedy word
//! wrap. Measurement lives on `Fonts` (see `fonts.rs`); this module turns
//! measured runs into painted glyphs on a surface.

use super::fonts::{Face, Fonts, sym};
use super::primitives::fill;
use krilla::geom::Point;
use krilla::surface::Surface;
use krilla::text::TextDirection;
use std::borrow::Cow;

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_str(
    s: &mut Surface,
    fonts: &Fonts,
    face: Face,
    size: f32,
    color: (u8, u8, u8),
    x: f32,
    baseline: f32,
    text: &str,
) {
    let text: Cow<str> = if face == Face::Symbols { sym(text) } else { Cow::Borrowed(text) };
    s.set_stroke(None);
    s.set_fill(Some(fill(color, 1.0)));
    s.draw_text(
        Point::from_xy(x, baseline),
        fonts.font(face),
        size,
        &text,
        false,
        TextDirection::LeftToRight,
    );
}

/// Letterspaced run (the engraved caps voice); measure with
/// `fonts.width(face, size, tracking, text)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tracked(
    s: &mut Surface,
    fonts: &Fonts,
    face: Face,
    size: f32,
    color: (u8, u8, u8),
    tracking: f32,
    x: f32,
    baseline: f32,
    text: &str,
) {
    let mut cx = x;
    let mut buf = [0u8; 4];
    for c in text.chars() {
        let cs: &str = c.encode_utf8(&mut buf);
        draw_str(s, fonts, face, size, color, cx, baseline, cs);
        cx += fonts.width(face, size, 0.0, cs) + tracking;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn center_str(
    s: &mut Surface,
    fonts: &Fonts,
    face: Face,
    size: f32,
    color: (u8, u8, u8),
    cx: f32,
    baseline: f32,
    text: &str,
) {
    let w = fonts.width(face, size, 0.0, text);
    draw_str(s, fonts, face, size, color, cx - w / 2.0, baseline, text);
}

/// Greedy word wrap against `width` points — incremental (each word is
/// measured once; a running line width replaces re-measuring the line).
pub(crate) fn wrap(fonts: &Fonts, face: Face, size: f32, width: f32, text: &str) -> Vec<String> {
    let space_w = fonts.width(face, size, 0.0, " ");
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_w = 0.0f32;
    for word in text.split_whitespace() {
        let word_w = fonts.width(face, size, 0.0, word);
        if line.is_empty() {
            line.push_str(word);
            line_w = word_w;
        } else if line_w + space_w + word_w <= width {
            line.push(' ');
            line.push_str(word);
            line_w += space_w + word_w;
        } else {
            lines.push(std::mem::take(&mut line));
            line.push_str(word);
            line_w = word_w;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
