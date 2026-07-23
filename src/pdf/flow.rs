//! The pages-2+ layout engine: a stream of measured [`FlowLine`]s (each with
//! its height, an orphan guard, and a deferred painter) that the driver
//! paginates. Builds the index of elements and the commentary; the driver
//! (`mod.rs`) slices the stream across pages.

use super::fonts::{Face, Fonts, glyph_face};
use super::palette::*;
use super::primitives::hline;
use super::text::{draw_str, draw_tracked, wrap};
use crate::contract::{ChartData, Excerpt};
use crate::i18n::Locale;
use krilla::surface::Surface;

/// `17°42' Cancer` — the template's fmtPos (U+2019 stands in for the
/// minutes prime; Libre Baskerville has no U+2032).
fn fmt_pos(chart: &ChartData, lon: f64) -> String {
    let within = lon.rem_euclid(360.0) % 30.0;
    let mut d = within.floor();
    let mut m = ((within - d) * 60.0).round();
    if m >= 60.0 {
        d += 1.0;
        m = 0.0;
    }
    let sign = &chart.signs[(lon.rem_euclid(360.0) / 30.0) as usize % 12];
    format!("{d}\u{b0}{m:02}\u{2019} {}", sign.name)
}

/// A line's painter, called with the line's top y.
pub(crate) type Painter<'c> = Box<dyn Fn(&mut Surface, &Fonts, f32) + 'c>;

/// One flowed line on pages 2+: height, an orphan guard, and its painter.
pub(crate) struct FlowLine<'c> {
    pub(crate) h: f32,
    /// Keep this line on the same page as the one after it.
    pub(crate) keep: bool,
    pub(crate) draw: Painter<'c>,
}

pub(crate) struct Frame {
    pub(crate) w: f32,
    pub(crate) margin: f32,
}

impl Frame {
    pub(crate) fn content_w(&self) -> f32 {
        self.w - 2.0 * self.margin
    }
}

/// A rubric — letterspaced small head flanked by rules.
fn rubric<'c>(frame: &Frame, title: &'c str) -> FlowLine<'c> {
    let margin = frame.margin;
    let cw = frame.content_w();
    FlowLine {
        h: 30.0,
        keep: true,
        draw: Box::new(move |s, fonts, y| {
            let size = 9.5;
            let tracking = size * 0.28;
            let text = title.to_uppercase();
            let w = fonts.width(Face::Regular, size, tracking, &text);
            let cx = margin + cw / 2.0;
            let baseline = y + 18.0;
            draw_tracked(s, fonts, Face::Regular, size, INK2, tracking, cx - w / 2.0, baseline, &text);
            let gap = 14.0;
            hline(s, margin, cx - w / 2.0 - gap, baseline - size * 0.32, LINE, 0.8);
            hline(s, cx + w / 2.0 + gap, margin + cw, baseline - size * 0.32, LINE, 0.8);
        }),
    }
}

fn spacer(h: f32) -> FlowLine<'static> {
    FlowLine { h, keep: false, draw: Box::new(|_, _, _| {}) }
}

/// Resolve a tag id to its chip: (glyph text, face, identity color). Serves
/// the folio chips AND the index rows — the one Rust home for "planets/
/// signs/aspects show their glyph, houses their roman label".
fn tag_chip(chart: &ChartData, tag: &str) -> Option<(String, Face, (u8, u8, u8))> {
    if let Some(p) = chart.planet(tag) {
        return Some((p.glyph.clone(), glyph_face(&p.glyph), BRASS));
    }
    if let Some(sign) = chart.signs.iter().find(|x| x.id == tag) {
        return Some((sign.glyph.clone(), Face::Symbols, VERDIGRIS));
    }
    if let Some(h) = chart.houses.iter().find(|x| x.id == tag) {
        return Some((h.label.clone(), Face::Regular, STEEL));
    }
    if let Some(a) = chart.aspects.iter().find(|x| x.id == tag) {
        let (pa, pb) = (chart.planet(&a.a)?, chart.planet(&a.b)?);
        return Some((format!("{} {} {}", pa.glyph, a.glyph, pb.glyph), Face::Symbols, OXBLOOD));
    }
    None
}

/// The passage block: folio line (time anchor + tag chips) and wrapped text.
fn passage<'c>(
    chart: &'c ChartData,
    ex: &'c Excerpt,
    fonts: &Fonts,
    frame: &Frame,
) -> Vec<FlowLine<'c>> {
    let margin = frame.margin;
    let cw = frame.content_w();
    let mut out: Vec<FlowLine<'c>> = Vec::new();

    // folio: chips laid out right-to-left from the right edge, time on the left
    let chips: Vec<(String, Face, (u8, u8, u8))> =
        ex.tags.iter().filter_map(|t| tag_chip(chart, t)).collect();
    let chip_size = 10.0;
    let mut positions = Vec::new();
    let mut right = margin + cw;
    for (text, face, color) in chips {
        let w = fonts.width(face, chip_size, 0.0, &text);
        right -= w;
        positions.push((right, text, face, color));
        right -= 10.0;
    }
    let time: &'c str = &ex.time;
    out.push(FlowLine {
        h: 16.0,
        keep: true,
        draw: Box::new(move |s, fonts, y| {
            let baseline = y + 11.0;
            if !time.is_empty() {
                draw_str(s, fonts, Face::Italic, 9.0, INK3, margin, baseline, time);
            }
            for (x, text, face, color) in &positions {
                draw_str(s, fonts, *face, chip_size, *color, *x, baseline, text);
            }
        }),
    });

    let size = 10.5;
    let leading = 16.0;
    for (i, line) in wrap(fonts, Face::Regular, size, cw, &ex.text).into_iter().enumerate() {
        out.push(FlowLine {
            h: leading,
            keep: i == 0, // the folio must not sit alone above a page break
            draw: Box::new(move |s, fonts, y| {
                draw_str(s, fonts, Face::Regular, size, INK, margin, y + size, &line);
            }),
        });
    }
    out.push(spacer(12.0));
    out
}

/// Everything that flows on pages 2+ — index of elements, then commentary.
pub(crate) fn build_flow<'c>(chart: &'c ChartData, fonts: &Fonts, frame: &Frame) -> Vec<FlowLine<'c>> {
    let margin = frame.margin;
    let cw = frame.content_w();
    let loc = Locale::parse(&chart.meta.locale);
    let mut flow: Vec<FlowLine<'c>> = Vec::new();

    flow.push(rubric(frame, loc.pdf().index_of_elements));
    flow.push(spacer(4.0));
    for p in &chart.planets {
        let Some((glyph, gface, _)) = tag_chip(chart, &p.id) else { continue };
        let name: &'c str = &p.name;
        let pos = fmt_pos(chart, p.lon);
        let house: &'c str = &chart.houses[(p.house as usize).saturating_sub(1) % 12].label;
        flow.push(FlowLine {
            h: 17.0,
            keep: false,
            draw: Box::new(move |s, fonts, y| {
                let b = y + 12.0;
                draw_str(s, fonts, gface, 11.0, BRASS, margin, b, &glyph);
                draw_str(s, fonts, Face::Regular, 10.5, INK, margin + 26.0, b, name);
                draw_str(s, fonts, Face::Regular, 10.5, INK2, margin + 130.0, b, &pos);
                let w = fonts.width(Face::Regular, 10.5, 0.0, house);
                draw_str(s, fonts, Face::Regular, 10.5, STEEL, margin + cw - w, b, house);
            }),
        });
    }
    if !chart.aspects.is_empty() {
        flow.push(spacer(10.0));
        for a in &chart.aspects {
            let Some((glyphs, _, _)) = tag_chip(chart, &a.id) else { continue };
            let name: &'c str = &a.name;
            flow.push(FlowLine {
                h: 16.0,
                keep: false,
                draw: Box::new(move |s, fonts, y| {
                    let b = y + 11.5;
                    draw_str(s, fonts, Face::Symbols, 10.5, OXBLOOD, margin, b, &glyphs);
                    draw_str(s, fonts, Face::Regular, 10.0, INK2, margin + 64.0, b, name);
                }),
            });
        }
    }

    if !chart.excerpts.is_empty() {
        flow.push(spacer(20.0));
        flow.push(rubric(frame, loc.pdf().commentary));
        flow.push(spacer(4.0));
        for ex in &chart.excerpts {
            flow.extend(passage(chart, ex, fonts, frame));
        }
    }
    flow
}
