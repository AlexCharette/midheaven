//! Stage 4b — the reading as a rich PDF: a cream-paper engraving of the same
//! material the HTML artifact carries (title plate, wheel, index,
//! commentary). Pure vector output via krilla; fonts are embedded and
//! subset, nothing external. The dark rendition stays the artifact's; paper
//! gets dark inks on warm cream.

mod fonts;
mod wheel;

use crate::contract::{ChartData, Excerpt};
use base64::Engine;
use fonts::{Face, Fonts, glyph_face, sym};
use krilla::Document;
use krilla::geom::{Path as KPath, PathBuilder, Point, Rect, Size};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::text::TextDirection;
use std::borrow::Cow;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum PageSize {
    #[default]
    A4,
    Letter,
}

impl PageSize {
    pub fn parse(s: &str) -> Result<PageSize, String> {
        match s.trim().to_lowercase().as_str() {
            "a4" => Ok(PageSize::A4),
            "letter" | "us-letter" => Ok(PageSize::Letter),
            other => Err(format!("unknown page size {other:?} (a4 or letter)")),
        }
    }

    /// Resolve an optional stored preference — absent means the default.
    pub fn from_pref(pref: Option<&str>) -> Result<PageSize, String> {
        pref.map_or(Ok(PageSize::default()), PageSize::parse)
    }

    fn dims(self) -> (f32, f32) {
        match self {
            PageSize::A4 => (595.276, 841.89),
            PageSize::Letter => (612.0, 792.0),
        }
    }
}

/// The engraved palette, pulled on paper: same identities as the artifact
/// (brass planets, verdigris signs, steel houses, oxblood aspects), darkened
/// for cream ground; hairlines are ink pre-blended onto the paper tone.
pub(crate) mod palette {
    pub const PAPER: (u8, u8, u8) = (244, 239, 226);
    pub const INK: (u8, u8, u8) = (43, 39, 33);
    pub const INK2: (u8, u8, u8) = (87, 80, 63);
    pub const INK3: (u8, u8, u8) = (122, 114, 92);
    pub const LINE: (u8, u8, u8) = (212, 207, 195);
    pub const HAIRLINE: (u8, u8, u8) = (184, 179, 168);
    pub const BRASS: (u8, u8, u8) = (138, 106, 28);
    pub const VERDIGRIS: (u8, u8, u8) = (30, 111, 82);
    pub const STEEL: (u8, u8, u8) = (60, 95, 150);
    pub const OXBLOOD: (u8, u8, u8) = (142, 52, 70);

    /// Element washes, pre-blended onto the paper tone (the artifact's
    /// rgba washes against cream instead of night).
    pub fn wash(element: &str) -> (u8, u8, u8) {
        match element {
            "fire" => (239, 224, 209),
            "earth" => (233, 224, 203),
            "air" => (238, 232, 213),
            _ => (229, 229, 224), // water
        }
    }
}
use palette::*;

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
fn draw_tracked(
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

fn hline(s: &mut Surface, x1: f32, x2: f32, y: f32, color: (u8, u8, u8), width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y);
    pb.line_to(x2, y);
    stroked(s, &pb.finish().expect("hline"), stroke(color, width, 1.0));
}

fn rect_stroke(s: &mut Surface, x: f32, y: f32, w: f32, h: f32, color: (u8, u8, u8), width: f32) {
    let mut pb = PathBuilder::new();
    pb.push_rect(Rect::from_xywh(x, y, w, h).expect("rect"));
    stroked(s, &pb.finish().expect("rect path"), stroke(color, width, 1.0));
}

/// Greedy word wrap against `width` points — incremental (each word is
/// measured once; a running line width replaces re-measuring the line).
fn wrap(fonts: &Fonts, face: Face, size: f32, width: f32, text: &str) -> Vec<String> {
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
type Painter<'c> = Box<dyn Fn(&mut Surface, &Fonts, f32) + 'c>;

/// One flowed line on pages 2+: height, an orphan guard, and its painter.
struct FlowLine<'c> {
    h: f32,
    /// Keep this line on the same page as the one after it.
    keep: bool,
    draw: Painter<'c>,
}

struct Frame {
    w: f32,
    margin: f32,
}

impl Frame {
    fn content_w(&self) -> f32 {
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
fn build_flow<'c>(chart: &'c ChartData, fonts: &Fonts, frame: &Frame) -> Vec<FlowLine<'c>> {
    let margin = frame.margin;
    let cw = frame.content_w();
    let mut flow: Vec<FlowLine<'c>> = Vec::new();

    flow.push(rubric(frame, "Index of Elements"));
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
        flow.push(rubric(frame, "Commentary"));
        flow.push(spacer(4.0));
        for ex in &chart.excerpts {
            flow.extend(passage(chart, ex, fonts, frame));
        }
    }
    flow
}

/// The compass ornament from the artifact's title plate, at `r` points.
fn compass(s: &mut Surface, cx: f32, cy: f32, r: f32) {
    let mut cross = PathBuilder::new();
    for (dx, dy) in [(0.0, 1.0), (1.0, 0.0)] {
        cross.move_to(cx - dx * r, cy - dy * r);
        cross.line_to(cx + dx * r, cy + dy * r);
    }
    stroked(s, &cross.finish().expect("compass rays"), stroke(BRASS, 0.9, 1.0));
    let d = r * 0.65;
    let mut diag = PathBuilder::new();
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0)] {
        diag.move_to(cx - sx * d, cy - sy * d);
        diag.line_to(cx + sx * d, cy + sy * d);
    }
    stroked(s, &diag.finish().expect("compass diagonals"), stroke(BRASS, 0.9, 0.55));
    for (rr, alpha) in [(r * 0.16, 1.0), (r * 0.34, 0.4)] {
        stroked(s, &circle_path(cx, cy, rr), stroke(BRASS, 0.9, alpha));
    }
}

/// Paper ground on every page.
fn paper(s: &mut Surface, w: f32, h: f32) {
    let mut pb = PathBuilder::new();
    pb.push_rect(Rect::from_xywh(0.0, 0.0, w, h).expect("page rect"));
    filled(s, &pb.finish().expect("paper"), fill(PAPER, 1.0));
}

/// The practitioner logo from `meta.logo` (a data URI) as a krilla image;
/// best-effort — anything undecodable falls back to the compass.
fn logo_image(meta_logo: &Option<String>) -> Option<(krilla::image::Image, f32)> {
    let uri = meta_logo.as_deref()?;
    let (head, b64) = uri.split_once(";base64,")?;
    let data: krilla::Data = base64::engine::general_purpose::STANDARD.decode(b64).ok()?.into();
    let image = match head {
        "data:image/png" => krilla::image::Image::from_png(data, true).ok()?,
        "data:image/jpeg" => krilla::image::Image::from_jpeg(data, true).ok()?,
        "data:image/webp" => krilla::image::Image::from_webp(data, true).ok()?,
        _ => return None, // svg logos stay HTML-only
    };
    let (w, h) = image.size();
    Some((image, w as f32 / h as f32))
}

/// Render the reading as PDF bytes.
pub fn render(chart: &ChartData, size: PageSize) -> Result<Vec<u8>, String> {
    let fonts = Fonts::new()?;
    let (w, h) = size.dims();
    let frame = Frame { w, margin: 64.0 };
    let cw = frame.content_w();
    let cx = w / 2.0;
    let mut doc = Document::new();
    let settings = || PageSettings::new(Size::from_wh(w, h).expect("page size"));

    // ---- page 1: title plate + the plate ----
    {
        let mut page = doc.start_page_with(settings());
        let mut s = page.surface();
        paper(&mut s, w, h);
        let mut y = frame.margin + 8.0;

        // ornament: the practitioner's mark when present, the compass otherwise
        match logo_image(&chart.meta.logo) {
            Some((image, aspect)) => {
                let lw = (34.0 * aspect).min(130.0);
                let lh = lw / aspect;
                s.push_transform(&krilla::geom::Transform::from_translate(cx - lw / 2.0, y));
                s.draw_image(image, Size::from_wh(lw, lh).expect("logo"));
                s.pop();
            }
            None => compass(&mut s, cx, y + 16.0, 17.0),
        }
        y += 46.0;

        center_str(&mut s, &fonts, Face::Italic, 12.0, INK2, cx, y + 12.0, "The Nativity of");
        y += 26.0;

        let name = chart.meta.name.to_uppercase();
        let mut nsize = 23.0;
        let tracking = |sz: f32| sz * 0.17;
        while fonts.width(Face::Regular, nsize, tracking(nsize), &name) > cw && nsize > 12.0 {
            nsize -= 1.0;
        }
        let nw = fonts.width(Face::Regular, nsize, tracking(nsize), &name);
        draw_tracked(&mut s, &fonts, Face::Regular, nsize, INK, tracking(nsize), cx - nw / 2.0, y + nsize, &name);
        y += nsize + 14.0;

        let details = if chart.meta.place.is_empty() {
            chart.meta.born.clone()
        } else {
            format!("{} \u{b7} {}", chart.meta.born, chart.meta.place)
        };
        center_str(&mut s, &fonts, Face::Italic, 10.5, INK2, cx, y + 10.0, &details);
        y += 20.0;

        if let Some(astrologer) = chart.meta.astrologer.as_deref() {
            let text = format!("PREPARED BY {}", astrologer.to_uppercase());
            let tw = fonts.width(Face::Regular, 7.5, 1.1, &text);
            draw_tracked(&mut s, &fonts, Face::Regular, 7.5, INK3, 1.1, cx - tw / 2.0, y + 9.0, &text);
            y += 18.0;
        }

        // double rule
        y += 10.0;
        let rw = 300.0f32.min(cw);
        hline(&mut s, cx - rw / 2.0, cx + rw / 2.0, y, HAIRLINE, 0.9);
        hline(&mut s, cx - rw / 2.0, cx + rw / 2.0, y + 5.0, LINE, 0.8);
        y += 26.0;

        // the plate: double frame + wheel, sized to what the page leaves us
        let caption_room = 64.0;
        let side = cw.min(h - frame.margin - caption_room - y - 12.0);
        let px = cx - side / 2.0;
        rect_stroke(&mut s, px, y, side, side, LINE, 0.8);
        rect_stroke(&mut s, px + 6.0, y + 6.0, side - 12.0, side - 12.0, HAIRLINE, 0.9);
        wheel::draw(&mut s, &fonts, chart, cx, y + side / 2.0, side / 2.0 - 22.0);
        y += side + 20.0;

        let caption = format!(
            "Fig. I. \u{2014} The natal figure of {}, calculated for {}{}. {} houses upon the {} zodiac.",
            chart.meta.name,
            chart.meta.born,
            if chart.meta.place.is_empty() { String::new() } else { format!(", {}", chart.meta.place) },
            chart.meta.system,
            chart.meta.zodiac.to_lowercase(),
        );
        for line in wrap(&fonts, Face::Italic, 9.5, cw * 0.9, &caption) {
            center_str(&mut s, &fonts, Face::Italic, 9.5, INK3, cx, y + 9.5, &line);
            y += 14.5;
        }

        s.finish();
        page.finish();
    }

    // ---- pages 2+: index + commentary flow (measure, then draw) ----
    let flow = build_flow(chart, &fonts, &frame);
    let bottom = h - frame.margin - 18.0;
    let mut i = 0;
    let mut page_no = 2;
    while i < flow.len() {
        // decide the page's slice first: fit by height, then pull back any
        // trailing keep-with-next run so folios never sit alone at a break
        let start = i;
        let mut y = frame.margin;
        while i < flow.len() && y + flow[i].h <= bottom {
            y += flow[i].h;
            i += 1;
        }
        while i > start + 1 && i < flow.len() && flow[i - 1].keep {
            i -= 1;
        }

        let mut page = doc.start_page_with(settings());
        let mut s = page.surface();
        paper(&mut s, w, h);
        let mut y = frame.margin;
        for line in &flow[start..i] {
            (line.draw)(&mut s, &fonts, y);
            y += line.h;
        }
        center_str(&mut s, &fonts, Face::Italic, 9.0, INK3, cx, h - 40.0, &format!("\u{b7} {page_no} \u{b7}"));
        s.finish();
        page.finish();
        page_no += 1;
    }

    doc.finish().map_err(|e| format!("cannot assemble the PDF: {e:?}"))
}

/// Render and write — the one entry point frontends call.
pub fn write_pdf(chart: &ChartData, size: PageSize, path: &Path) -> Result<(), String> {
    let bytes = render(chart, size)?;
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::catalog::{ASPECT_TYPES, PLANETS, SIGNS_ALL};

    fn chart_fixture() -> ChartData {
        let input = crate::chart::BirthInput {
            name: "Mira Holt".into(),
            date: "1990-07-13".parse().unwrap(),
            time: "14:30:00".parse().unwrap(),
            lat: 52.52,
            lon: 13.405,
            tz: chrono_tz::Europe::Berlin,
            place: "Berlin, Germany".into(),
        };
        let mut chart = crate::chart::compute_chart(&input).unwrap();
        chart.excerpts.push(Excerpt {
            id: "x1".into(),
            time: "00:00:12".into(),
            span: [0, 24],
            text: "The sun rules this chart and it keeps returning to the tenth house, \
                   again and again, whenever the year turns."
                .into(),
            tags: vec!["planet:sun".into(), "house:10".into()],
        });
        chart
    }

    #[test]
    fn symbol_font_covers_the_whole_catalog() {
        let fonts = Fonts::new().unwrap();
        for (_, _, glyph, _) in PLANETS {
            assert!(fonts.covers(Face::Symbols, glyph), "planet glyph {glyph}");
        }
        for (_, glyph, _, _) in SIGNS_ALL {
            assert!(fonts.covers(Face::Symbols, glyph), "sign glyph {glyph}");
        }
        for (_, glyph, _, _, _) in ASPECT_TYPES {
            assert!(fonts.covers(Face::Symbols, glyph), "aspect glyph {glyph}");
        }
        assert!(fonts.covers(Face::Symbols, "\u{2736}\u{261E}\u{b7}\u{b0} "));
        // everything the text faces set outside plain words
        for face in [Face::Regular, Face::Italic] {
            assert!(fonts.covers(face, "0123456789\u{b0}\u{2019}\u{b7}\u{2014}IVX:"), "text face");
        }
    }

    #[test]
    fn renders_a_wellformed_pdf_at_both_sizes() {
        let chart = chart_fixture();
        for size in [PageSize::A4, PageSize::Letter] {
            let bytes = render(&chart, size).unwrap();
            assert!(bytes.starts_with(b"%PDF-"), "{size:?} magic");
            let tail = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(64)..]).to_string();
            assert!(tail.contains("%%EOF"), "{size:?} trailer");
            assert!(bytes.len() > 20_000, "{size:?} suspiciously small: {}", bytes.len());
        }
    }

    #[test]
    fn page_size_parses_and_defaults() {
        assert_eq!(PageSize::parse("A4").unwrap(), PageSize::A4);
        assert_eq!(PageSize::parse("letter").unwrap(), PageSize::Letter);
        assert!(PageSize::parse("a5").is_err());
        assert_eq!(PageSize::from_pref(None).unwrap(), PageSize::A4);
        assert_eq!(PageSize::from_pref(Some("letter")).unwrap(), PageSize::Letter);
    }
}
