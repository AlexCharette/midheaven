//! The pages-2+ layout engine: a stream of measured [`FlowLine`]s (each with
//! its height, an orphan guard, and a deferred painter) that the driver
//! paginates. Builds the index of elements and the commentary; the driver
//! (`mod.rs`) slices the stream across pages.

use super::fonts::{Face, Fonts, glyph_face};
use super::palette::*;
use super::primitives::hline;
use super::text::{draw_str, draw_tracked, wrap};
use crate::contract::{Body, ChartData, Excerpt};
use crate::i18n::Locale;
use krilla::surface::Surface;
use std::ops::Range;

/// `17°42' Cancer` — the body's derived position, read from the contract rather
/// than re-derived here (U+2019 stands in for the minutes prime; Libre
/// Baskerville has no U+2032).
fn fmt_pos(chart: &ChartData, body: &Body) -> String {
    let sign = &chart.signs[body.sign as usize % 12];
    format!("{}\u{b0}{:02}\u{2019} {}", body.deg, body.min, sign.name)
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

impl FlowLine<'_> {
    pub(crate) fn measure(&self) -> Measure {
        Measure { h: self.h, keep: self.keep }
    }
}

/// Everything pagination needs to know about a line, and nothing else.
///
/// Separate from [`FlowLine`] because a `FlowLine` borrows the chart it paints
/// (`&'c str` into names and passage text), so a test could not build one
/// without running the ephemeris. A `Measure` is two `Copy` fields, which is
/// what makes [`slices`] testable at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Measure {
    pub(crate) h: f32,
    pub(crate) keep: bool,
}

/// How a flow divides into pages: half-open ranges into `lines`, in order.
///
/// Two rules, applied in that order:
///
///  1. **Fit.** Take lines while they fit in `room`.
///  2. **Pull back.** Give back any trailing keep-with-next run, so a rubric or
///     a passage's folio line never sits alone above a break.
///
/// Progress wins over rule 2: if pulling back would empty the page, one line
/// stays and its keep is broken — a page that took nothing would loop forever.
///
/// A line taller than `room` is refused rather than skipped. Every height in
/// `build_flow` is a literal well under a page, so this is unreachable today by
/// construction; before this function existed it was unreachable *and* would
/// have spun the pagination loop emitting blank pages.
///
/// Split out from the loop that starts pages so the decision is a value. It used
/// to be made and spent inside that loop, which meant the only way to ask how a
/// reading paginated was to open the PDF.
pub(crate) fn slices(lines: &[Measure], room: f32) -> Result<Vec<Range<usize>>, String> {
    let mut pages = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let start = i;
        let mut used = 0.0;
        while i < lines.len() && used + lines[i].h <= room {
            used += lines[i].h;
            i += 1;
        }
        if i == start {
            return Err(format!(
                "a flowed line is {:.1}pt tall; a page has room for {room:.1}pt",
                lines[start].h
            ));
        }
        while i > start + 1 && i < lines.len() && lines[i - 1].keep {
            i -= 1;
        }
        pages.push(start..i);
    }
    Ok(pages)
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
        let pos = fmt_pos(chart, p);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A line of height `h` that does not hold onto the next one.
    fn line(h: f32) -> Measure {
        Measure { h, keep: false }
    }
    /// A line that must not be the last on its page — a rubric, or a passage's
    /// folio line.
    fn kept(h: f32) -> Measure {
        Measure { h, keep: true }
    }

    fn pages(lines: &[Measure], room: f32) -> Vec<Range<usize>> {
        slices(lines, room).expect("these fixtures all fit")
    }

    #[test]
    fn an_empty_flow_makes_no_pages() {
        assert!(pages(&[], 100.0).is_empty());
    }

    #[test]
    fn lines_fill_a_page_up_to_its_room_and_no_further() {
        let flow = [line(30.0), line(30.0), line(30.0), line(30.0)];
        // Exactly three fit in 90pt; the fourth starts the next page.
        assert_eq!(pages(&flow, 90.0), vec![0..3, 3..4]);
        // One point more room changes nothing — heights are exact, not rounded.
        assert_eq!(pages(&flow, 90.9), vec![0..3, 3..4]);
        // Enough for all four is one page.
        assert_eq!(pages(&flow, 120.0), vec![0..4]);
    }

    #[test]
    fn every_line_lands_on_exactly_one_page_in_order() {
        let flow: Vec<Measure> = (0..40).map(|i| if i % 7 == 0 { kept(17.0) } else { line(16.0) }).collect();
        let pages = pages(&flow, 100.0);
        let mut expected = 0;
        for p in &pages {
            assert_eq!(p.start, expected, "pages must tile the flow with no gap or overlap");
            assert!(p.end > p.start, "no page may be empty");
            expected = p.end;
        }
        assert_eq!(expected, flow.len(), "every line lands somewhere");
    }

    #[test]
    fn no_page_holds_more_than_its_room() {
        let flow: Vec<Measure> = (0..40).map(|i| line(10.0 + (i % 5) as f32 * 3.0)).collect();
        let room = 100.0;
        for p in pages(&flow, room) {
            let used: f32 = flow[p.clone()].iter().map(|m| m.h).sum();
            assert!(used <= room, "page {p:?} used {used}pt of {room}pt");
        }
    }

    /// The orphan rule: a rubric introduces the row after it, so it must not be
    /// the last thing on a page.
    #[test]
    fn a_keep_with_next_line_is_pulled_to_the_page_that_follows_it() {
        // 30 + 30 + 30 fills the page exactly, but the third is `keep`.
        let flow = [line(30.0), line(30.0), kept(30.0), line(30.0)];
        assert_eq!(pages(&flow, 90.0), vec![0..2, 2..4], "the kept line moves down with its row");
    }

    #[test]
    fn a_whole_trailing_run_of_kept_lines_moves_together() {
        let flow = [line(20.0), kept(20.0), kept(20.0), kept(20.0), line(20.0)];
        // 100pt would take all five; the run of three kept lines goes down whole.
        assert_eq!(pages(&flow, 80.0), vec![0..1, 1..5]);
    }

    #[test]
    fn a_kept_line_at_the_very_end_of_the_flow_is_left_alone() {
        // Nothing follows, so "keep with next" has nothing to hold onto and must
        // not push the last line onto a page of its own.
        let flow = [line(30.0), line(30.0), kept(30.0)];
        assert_eq!(pages(&flow, 90.0), vec![0..3]);
    }

    /// Progress beats the orphan rule: pulling back to nothing would leave a
    /// page empty and the flow never advancing.
    #[test]
    fn a_page_of_nothing_but_kept_lines_still_places_one() {
        let flow = [kept(30.0), kept(30.0), kept(30.0), line(30.0)];
        let pages = pages(&flow, 90.0);
        assert_eq!(pages[0], 0..1, "one line stays rather than the page taking none");
        let mut at = 0;
        for p in &pages {
            assert_eq!(p.start, at);
            at = p.end;
        }
        assert_eq!(at, flow.len());
    }

    /// A line taller than the page can never be placed. Before the decision was
    /// separated this returned an empty slice forever and the driver emitted
    /// blank pages until it ran out of memory.
    #[test]
    fn a_line_taller_than_the_page_is_refused_rather_than_looping() {
        let err = slices(&[line(20.0), line(500.0)], 100.0).unwrap_err();
        assert!(err.contains("500.0pt"), "{err}");
        assert!(err.contains("100.0pt"), "{err}");
    }

    #[test]
    fn a_flow_of_one_line_is_one_page() {
        assert_eq!(pages(&[line(10.0)], 100.0), vec![0..1]);
        assert_eq!(pages(&[kept(10.0)], 100.0), vec![0..1]);
    }
}
