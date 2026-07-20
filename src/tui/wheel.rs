//! The hero plate: the natal wheel drawn on a braille canvas — rings, cusp
//! spokes, ASC/MC axes, aspect chords, planet glyphs at true longitudes.
//! Geometry matches the HTML viewer: ASC on the left, longitudes increasing
//! counterclockwise.

use super::theme;
use astro::contract::ChartData;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::Marker;
use ratatui::text::Span;
use ratatui::widgets::canvas::{Canvas, Circle, Context, Line as CanvasLine};

const R_OUTER: f64 = 1.0;
const R_SIGN_IN: f64 = 0.78;
const R_SIGN_GLYPH: f64 = 0.89;
const R_PLANET: f64 = 0.58;
const R_CHORD: f64 = 0.42;
const R_HUB: f64 = 0.24;
const R_HOUSE_LBL: f64 = 0.33;

pub fn render(chart: &ChartData, area: Rect, frame: &mut Frame) {
    let asc = chart.axes.asc;
    // Screen angle: ASC at left, ecliptic longitude increasing ccw.
    let pt = move |lon: f64, r: f64| -> (f64, f64) {
        let a = std::f64::consts::PI + (lon - asc).to_radians();
        (r * a.cos(), r * a.sin())
    };

    // Keep the circle round: braille cells are 2×4 dots, so world units per
    // dot must match across axes: x_range/(2w) == y_range/(4h).
    let y_span = 1.25;
    let x_span = if area.height == 0 {
        y_span
    } else {
        y_span * (f64::from(area.width) / (2.0 * f64::from(area.height))).max(0.2)
    };

    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-x_span, x_span])
        .y_bounds([-y_span, y_span])
        .paint(move |ctx| paint(ctx, chart, &pt));
    frame.render_widget(canvas, area);
}

fn paint(ctx: &mut Context, chart: &ChartData, pt: &impl Fn(f64, f64) -> (f64, f64)) {
    for r in [R_OUTER, R_SIGN_IN, R_HUB] {
        ctx.draw(&Circle { x: 0.0, y: 0.0, radius: r, color: theme::HAIRLINE });
    }

    // house cusp spokes
    for &cusp in &chart.house_cusps {
        let (x1, y1) = pt(cusp, R_HUB);
        let (x2, y2) = pt(cusp, R_SIGN_IN);
        ctx.draw(&CanvasLine { x1, y1, x2, y2, color: theme::LINE });
    }

    // ASC/MC axes, slightly proud of the outer ring
    for (lon, label) in [
        (chart.axes.asc, "AC"),
        (chart.axes.mc, "MC"),
        (chart.axes.asc + 180.0, "DC"),
        (chart.axes.mc + 180.0, "IC"),
    ] {
        let (x1, y1) = pt(lon, R_HUB);
        let (x2, y2) = pt(lon, R_OUTER);
        ctx.draw(&CanvasLine { x1, y1, x2, y2, color: theme::HAIRLINE });
        let (lx, ly) = pt(lon, R_OUTER + 0.12);
        ctx.print(lx, ly, Span::styled(label, Style::new().fg(theme::INK3)));
    }

    // aspect chords under the glyphs
    let lon_of = |id: &str| chart.planets.iter().find(|p| p.id == id).map(|p| p.lon);
    for a in &chart.aspects {
        if let (Some(la), Some(lb)) = (lon_of(&a.a), lon_of(&a.b)) {
            let (x1, y1) = pt(la, R_CHORD);
            let (x2, y2) = pt(lb, R_CHORD);
            ctx.draw(&CanvasLine { x1, y1, x2, y2, color: theme::OXBLOOD });
        }
    }

    // sign glyphs on the band
    for (i, s) in chart.signs.iter().enumerate() {
        let (x, y) = pt(i as f64 * 30.0 + 15.0, R_SIGN_GLYPH);
        ctx.print(x, y, Span::styled(s.glyph.clone(), Style::new().fg(theme::VERDIGRIS)));
    }

    // house numerals
    for (i, h) in chart.houses.iter().enumerate() {
        let cusp = chart.house_cusps[i];
        let next = chart.house_cusps[(i + 1) % 12];
        let sweep = (next - cusp).rem_euclid(360.0);
        let sweep = if sweep == 0.0 { 30.0 } else { sweep };
        let (x, y) = pt(cusp + sweep / 2.0, R_HOUSE_LBL);
        ctx.print(x, y, Span::styled(h.label.clone(), Style::new().fg(theme::STEEL)));
    }

    // planet glyphs, nudged inward when crowded (same rule as the viewer)
    let mut by_lon: Vec<_> = chart.planets.iter().collect();
    by_lon.sort_by(|a, b| a.lon.total_cmp(&b.lon));
    let mut prev_lon: Option<f64> = None;
    let mut prev_r = R_PLANET;
    for p in by_lon {
        let crowded = prev_lon.is_some_and(|pl| {
            let d = (p.lon - pl).abs();
            d.min(360.0 - d) < 9.0
        });
        let r = if crowded { (prev_r - 0.11).max(0.30) } else { R_PLANET };
        prev_lon = Some(p.lon);
        prev_r = r;
        let (x, y) = pt(p.lon, r);
        ctx.print(x, y, Span::styled(p.glyph.clone(), Style::new().fg(theme::BRASS)));
    }
}
