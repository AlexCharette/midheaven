//! Stage 4b — the reading as a rich PDF: a lighter rendition of the same
//! material the HTML artifact carries (title plate, wheel, index,
//! commentary). Pure vector output via krilla; fonts are embedded and
//! subset, nothing external. The dark rendition stays the artifact's; paper
//! gets dark inks on warm cream.
//!
//! The module is split by concern: [`palette`] (the color table),
//! [`primitives`] (paint/path helpers), [`text`] (typography), [`wheel`] (the
//! chart plate), and [`flow`] (the pages-2+ layout engine). This file is the
//! driver: page geometry, ornaments, and the [`render`] entry point.

mod flow;
mod fonts;
mod palette;
mod primitives;
mod text;
mod wheel;

use crate::contract::ChartData;
use crate::i18n::Locale;
use base64::Engine;
use flow::{Frame, build_flow};
use fonts::{Face, Fonts};
use krilla::Document;
use krilla::geom::{PathBuilder, Rect, Size};
use krilla::page::PageSettings;
use krilla::surface::Surface;
use palette::*;
use primitives::{circle_path, fill, filled, hline, rect_stroke, stroke, stroked};
use std::path::Path;
use text::{center_str, draw_tracked, wrap};

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

/// The page margin — the content box the title plate and the flow both live in.
const MARGIN: f32 = 64.0;

/// Title-plate layout (page 1). The page reads top-to-bottom as a cursor `y`
/// advanced past each band; these name the bands' type sizes, baseline drops,
/// and advances so the layout reads as rhythm rather than bare literals.
mod plate {
    /// Gap below the top margin before the ornament.
    pub const TOP_PAD: f32 = 8.0;

    // ornament (practitioner logo, or the compass fallback)
    pub const ORNAMENT_ADVANCE: f32 = 46.0;
    pub const COMPASS_DROP: f32 = 16.0; // compass centre below the cursor
    pub const COMPASS_RADIUS: f32 = 17.0;
    pub const LOGO_UNIT: f32 = 34.0; // logo width per unit of aspect ratio
    pub const LOGO_MAX_W: f32 = 130.0;

    // "nativity of" super-title
    pub const SUPERTITLE_SIZE: f32 = 12.0;
    pub const SUPERTITLE_BASELINE: f32 = 12.0;
    pub const SUPERTITLE_ADVANCE: f32 = 26.0;

    // the name, auto-shrunk to fit the content width
    pub const NAME_MAX_SIZE: f32 = 23.0;
    pub const NAME_MIN_SIZE: f32 = 12.0;
    pub const NAME_SHRINK_STEP: f32 = 1.0;
    pub const NAME_TRACKING_RATIO: f32 = 0.17;
    pub const NAME_ADVANCE_PAD: f32 = 14.0; // added to the fitted size

    // born · place detail line
    pub const DETAIL_SIZE: f32 = 10.5;
    pub const DETAIL_BASELINE: f32 = 10.0;
    pub const DETAIL_ADVANCE: f32 = 20.0;

    // "prepared by …" byline (present only when branded)
    pub const BYLINE_SIZE: f32 = 7.5;
    pub const BYLINE_TRACKING: f32 = 1.1;
    pub const BYLINE_BASELINE: f32 = 9.0;
    pub const BYLINE_ADVANCE: f32 = 18.0;

    // the double rule under the header
    pub const RULE_TOP_PAD: f32 = 10.0;
    pub const RULE_MAX_W: f32 = 300.0;
    pub const RULE_GAP: f32 = 5.0; // between the two rules
    pub const RULE_ADVANCE: f32 = 26.0;

    // the plate: double frame + wheel, sized to what the page leaves
    pub const CAPTION_ROOM: f32 = 64.0; // vertical room reserved for the caption
    pub const BOTTOM_SLACK: f32 = 12.0;
    pub const FRAME_INSET: f32 = 6.0; // inner frame inset from the outer
    pub const WHEEL_INSET: f32 = 22.0; // label ring inside the inner frame
    pub const ADVANCE_PAD: f32 = 20.0; // below the plate before the caption

    // the figure caption
    pub const CAPTION_SIZE: f32 = 9.5;
    pub const CAPTION_WIDTH_RATIO: f32 = 0.9;
    pub const CAPTION_BASELINE: f32 = 9.5;
    pub const CAPTION_LEADING: f32 = 14.5;
}

/// Flowed pages (2+) layout.
mod flowed {
    pub const FOOTER_ROOM: f32 = 18.0; // bottom margin the folio sits in
    pub const FOOTER_DROP: f32 = 40.0; // folio baseline above the page bottom
    pub const FOOTER_SIZE: f32 = 9.0;
}

fn page_settings(w: f32, h: f32) -> PageSettings {
    PageSettings::new(Size::from_wh(w, h).expect("page size"))
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
    let loc = Locale::parse(&chart.meta.locale);
    let fonts = Fonts::new(loc)?;
    let (w, h) = size.dims();
    let frame = Frame { w, margin: MARGIN };
    let mut doc = Document::new();

    render_title_page(&mut doc, chart, &fonts, loc, &frame, h);
    paginate(&mut doc, chart, &fonts, &frame, h);

    doc.finish().map_err(|e| format!("cannot assemble the PDF: {e:?}"))
}

/// Page 1: the title plate — header (ornament, super-title, name, details,
/// byline), a double rule, the framed wheel, and the figure caption.
fn render_title_page(
    doc: &mut Document,
    chart: &ChartData,
    fonts: &Fonts,
    loc: Locale,
    frame: &Frame,
    h: f32,
) {
    use plate::*;
    let w = frame.w;
    let cw = frame.content_w();
    let cx = w / 2.0;
    let mut page = doc.start_page_with(page_settings(w, h));
    let mut s = page.surface();
    paper(&mut s, w, h);
    let mut y = frame.margin + TOP_PAD;

    // ornament: the practitioner's mark when present, the compass otherwise
    match logo_image(&chart.meta.logo) {
        Some((image, aspect)) => {
            let lw = (LOGO_UNIT * aspect).min(LOGO_MAX_W);
            let lh = lw / aspect;
            s.push_transform(&krilla::geom::Transform::from_translate(cx - lw / 2.0, y));
            s.draw_image(image, Size::from_wh(lw, lh).expect("logo"));
            s.pop();
        }
        None => compass(&mut s, cx, y + COMPASS_DROP, COMPASS_RADIUS),
    }
    y += ORNAMENT_ADVANCE;

    center_str(&mut s, fonts, Face::Italic, SUPERTITLE_SIZE, INK2, cx, y + SUPERTITLE_BASELINE, loc.pdf().nativity_of);
    y += SUPERTITLE_ADVANCE;

    let name = chart.meta.name.to_uppercase();
    let mut nsize = NAME_MAX_SIZE;
    let tracking = |sz: f32| sz * NAME_TRACKING_RATIO;
    while fonts.width(Face::Regular, nsize, tracking(nsize), &name) > cw && nsize > NAME_MIN_SIZE {
        nsize -= NAME_SHRINK_STEP;
    }
    let nw = fonts.width(Face::Regular, nsize, tracking(nsize), &name);
    draw_tracked(&mut s, fonts, Face::Regular, nsize, INK, tracking(nsize), cx - nw / 2.0, y + nsize, &name);
    y += nsize + NAME_ADVANCE_PAD;

    let details = if chart.meta.place.is_empty() {
        chart.meta.born.clone()
    } else {
        format!("{} \u{b7} {}", chart.meta.born, chart.meta.place)
    };
    center_str(&mut s, fonts, Face::Italic, DETAIL_SIZE, INK2, cx, y + DETAIL_BASELINE, &details);
    y += DETAIL_ADVANCE;

    if let Some(astrologer) = chart.meta.astrologer.as_deref() {
        let text = format!("{} {}", loc.pdf().prepared_by.to_uppercase(), astrologer.to_uppercase());
        let tw = fonts.width(Face::Regular, BYLINE_SIZE, BYLINE_TRACKING, &text);
        draw_tracked(&mut s, fonts, Face::Regular, BYLINE_SIZE, INK3, BYLINE_TRACKING, cx - tw / 2.0, y + BYLINE_BASELINE, &text);
        y += BYLINE_ADVANCE;
    }

    // double rule
    y += RULE_TOP_PAD;
    let rw = RULE_MAX_W.min(cw);
    hline(&mut s, cx - rw / 2.0, cx + rw / 2.0, y, HAIRLINE, 0.9);
    hline(&mut s, cx - rw / 2.0, cx + rw / 2.0, y + RULE_GAP, LINE, 0.8);
    y += RULE_ADVANCE;

    // the plate: double frame + wheel, sized to what the page leaves us
    let side = cw.min(h - frame.margin - CAPTION_ROOM - y - BOTTOM_SLACK);
    let px = cx - side / 2.0;
    rect_stroke(&mut s, px, y, side, side, LINE, 0.8);
    rect_stroke(&mut s, px + FRAME_INSET, y + FRAME_INSET, side - 2.0 * FRAME_INSET, side - 2.0 * FRAME_INSET, HAIRLINE, 0.9);
    wheel::draw(&mut s, fonts, chart, cx, y + side / 2.0, side / 2.0 - WHEEL_INSET);
    y += side + ADVANCE_PAD;

    let caption = loc.pdf_figure_caption(
        &chart.meta.name,
        &chart.meta.born,
        &chart.meta.place,
        &chart.meta.system,
        &chart.meta.zodiac,
    );
    for line in wrap(fonts, Face::Italic, CAPTION_SIZE, cw * CAPTION_WIDTH_RATIO, &caption) {
        center_str(&mut s, fonts, Face::Italic, CAPTION_SIZE, INK3, cx, y + CAPTION_BASELINE, &line);
        y += CAPTION_LEADING;
    }

    s.finish();
    page.finish();
}

/// Pages 2+: measure the flow once, then lay it out page by page — fit lines
/// by height, pull back any trailing keep-with-next run so a folio never sits
/// alone at a break, and stamp the folio number.
fn paginate(doc: &mut Document, chart: &ChartData, fonts: &Fonts, frame: &Frame, h: f32) {
    let w = frame.w;
    let cx = w / 2.0;
    let flow = build_flow(chart, fonts, frame);
    let bottom = h - frame.margin - flowed::FOOTER_ROOM;
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

        let mut page = doc.start_page_with(page_settings(w, h));
        let mut s = page.surface();
        paper(&mut s, w, h);
        let mut y = frame.margin;
        for line in &flow[start..i] {
            (line.draw)(&mut s, fonts, y);
            y += line.h;
        }
        center_str(&mut s, fonts, Face::Italic, flowed::FOOTER_SIZE, INK3, cx, h - flowed::FOOTER_DROP, &format!("\u{b7} {page_no} \u{b7}"));
        s.finish();
        page.finish();
        page_no += 1;
    }
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
    use crate::contract::Excerpt;

    fn chart_fixture() -> ChartData {
        let input = crate::chart::BirthInput {
            name: "Mira Holt".into(),
            date: "1990-07-13".parse().unwrap(),
            time: "14:30:00".parse().unwrap(),
            lat: 52.52,
            lon: 13.405,
            tz: chrono_tz::Europe::Berlin,
            place: "Berlin, Germany".into(),
            locale: crate::i18n::Locale::En,
            house_system: xalen_houses::HouseSystem::WholeSign,
            ayanamsa: None,
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
        let fonts = Fonts::new(Locale::En).unwrap();
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
    fn russian_body_font_covers_cyrillic() {
        // The Russian body face must render Cyrillic (upper + lower) plus the
        // localized element names; the Latin font can't, which is why ru swaps.
        let fonts = Fonts::new(Locale::Ru).unwrap();
        for face in [Face::Regular, Face::Italic] {
            assert!(fonts.covers(face, "АБВГ абвгдеёжз Солнце Рак дом"), "cyrillic text face");
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
    fn renders_a_russian_reading() {
        // A ru chart must render without error and produce a well-formed PDF.
        let input = crate::chart::BirthInput {
            name: "Мира Холт".into(),
            date: "1990-07-13".parse().unwrap(),
            time: "14:30:00".parse().unwrap(),
            lat: 52.52,
            lon: 13.405,
            tz: chrono_tz::Europe::Berlin,
            place: "Берлин, Германия".into(),
            locale: Locale::Ru,
            house_system: xalen_houses::HouseSystem::WholeSign,
            ayanamsa: None,
        };
        let mut chart = crate::chart::compute_chart(&input).unwrap();
        chart.excerpts.push(Excerpt {
            id: "x1".into(),
            time: "00:00:12".into(),
            span: [0, 10],
            text: "Ваше солнце в раке освещает десятый дом.".into(),
            tags: vec!["planet:sun".into(), "house:10".into()],
        });
        let bytes = render(&chart, PageSize::A4).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 20_000);
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
