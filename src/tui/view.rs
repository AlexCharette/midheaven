//! Pure rendering of the Model — nothing here mutates state.

use super::model::{FIELDS, Field, Form, Job, Mode, Model, Reading, Screen};
use super::{theme, wheel};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

pub fn view(model: &Model, frame: &mut Frame) {
    let area = frame.area();
    // the indigo field behind everything
    frame.render_widget(Block::default().style(theme::base()), area);
    let [body, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    match &model.screen {
        Screen::Form(form) => view_form(form, body, frame),
        Screen::Reading(reading) => view_reading(reading, body, frame),
    }
    view_status(model, status, frame);
}

fn rubric_line(text: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled("──── ", theme::hairline()),
        theme::rubric(text),
        Span::styled(" ────", theme::hairline()),
    ])
    .alignment(Alignment::Center)
}

// ---------------------------------------------------------------- form ----

fn view_form(form: &Form, area: Rect, frame: &mut Frame) {
    let width = 66.min(area.width);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let col = Rect { x, width, ..area };

    let mut lines: Vec<Line> = vec![
        Line::raw(""),
        Line::from(Span::styled("✶", theme::ink2())).alignment(Alignment::Center),
        Line::from(Span::styled("The Nativity Desk", theme::apparatus())).alignment(Alignment::Center),
        Line::from(Span::styled(
            theme::letterspace("ASTRO"),
            Style::new().fg(theme::INK).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Line::from(Span::styled(
            "enter the birth data; the chart computes entirely offline",
            theme::apparatus(),
        ))
        .alignment(Alignment::Center),
        Line::raw(""),
    ];

    let mut place_row: u16 = 0;
    for field in FIELDS {
        let focused = form.focus == field;
        if field == Field::Place {
            place_row = lines.len() as u16; // dropdown anchor: the row being pushed
        }
        let label = format!("{:>10}  ", field.label());
        let value = display_value(form, field);
        let mut spans = vec![
            Span::styled(theme::marker(focused), theme::ink2()),
            Span::styled(label, theme::apparatus()),
            Span::styled(value.clone(), theme::highlight(focused)),
        ];
        if focused {
            spans.push(Span::styled("▏", Style::new().fg(theme::BRASS)));
            if value.is_empty() {
                spans.push(Span::styled(field.hint(), theme::hairline().italic()));
            }
        }
        lines.push(Line::from(spans));
        if let Some((f, err)) = &form.error
            && *f == field
        {
            lines.push(Line::from(vec![
                Span::raw("              "),
                Span::styled(format!("✗ {err}"), theme::error()),
            ]));
        }
        lines.push(Line::raw(""));
    }

    lines.push(Line::from(Span::styled(
        "tab · move   enter · next / choose   F5 · compute the figure   ctrl-c · leave",
        theme::apparatus(),
    ))
    .alignment(Alignment::Center));

    frame.render_widget(Paragraph::new(lines), col);

    // the gazetteer dropdown floats under the place field, anchored to the
    // row recorded while the form lines were built
    if !form.suggestions.is_empty() && form.focus == Field::Place {
        let place_row = col.y + place_row;
        let drop = Rect {
            x: col.x + 14,
            y: place_row + 1,
            width: width.saturating_sub(16),
            height: (form.suggestions.len() as u16 + 2).min(area.height.saturating_sub(place_row + 1)),
        };
        frame.render_widget(Clear, drop);
        let items: Vec<Line> = form
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let current = form.sel == Some(i);
                Line::from(vec![
                    Span::styled(theme::marker(current), theme::ink2()),
                    Span::styled(p.label(), theme::highlight(current)),
                    Span::styled(format!("  {}", p.tz), theme::apparatus()),
                ])
            })
            .collect();
        frame.render_widget(
            Paragraph::new(items).style(theme::base()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::hairline()),
            ),
            drop,
        );
    }
}

fn display_value(form: &Form, field: Field) -> String {
    if field == Field::Place
        && let Some(p) = form.picked
    {
        return format!("{} · {}", p.label(), p.tz);
    }
    form.value(field).to_string()
}

// ------------------------------------------------------------- reading ----

fn view_reading(reading: &Reading, area: Rect, frame: &mut Frame) {
    let [plate_area, right] =
        Layout::horizontal([Constraint::Percentage(44), Constraint::Min(40)]).areas(area);

    view_plate(reading, plate_area, frame);

    let rows = reading.columns.iter().map(|c| c.entries.len()).max().unwrap_or(0);
    let index_height = (rows as u16 + 3).min(area.height / 2);
    let [index_area, apparatus, commentary] = Layout::vertical([
        Constraint::Length(index_height),
        Constraint::Length(1),
        Constraint::Min(3),
    ])
    .areas(right);

    let visible = reading.visible();
    view_index(reading, index_area, frame);
    view_apparatus(reading, visible.len(), apparatus, frame);
    view_commentary(reading, &visible, commentary, frame);
}

fn view_plate(reading: &Reading, area: Rect, frame: &mut Frame) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme::hairline())
        .title(
            Line::from(Span::styled(" Fig. I ", theme::ink2())).alignment(Alignment::Center),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [wheel_area, caption] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).areas(inner);
    wheel::render(&reading.chart, wheel_area, frame);
    let m = &reading.chart.meta;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("the natal figure of {}, {} — {}", m.name, m.born, m.place),
            theme::apparatus(),
        )))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        caption,
    );
}

fn view_index(reading: &Reading, area: Rect, frame: &mut Frame) {
    let [rubric_area, cols_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(Paragraph::new(rubric_line("Index of Elements")), rubric_area);

    let col_areas: [Rect; 4] = Layout::horizontal([
        Constraint::Percentage(26),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(34),
    ])
    .areas(cols_area);

    for (c, (column, col_area)) in reading.columns.iter().zip(col_areas).enumerate() {
        let mut lines = vec![Line::from(Span::styled(column.head, theme::apparatus()))];
        for (r, entry) in column.entries.iter().enumerate() {
            let selected = reading.selected.contains(&entry.tag);
            let mut name_style = theme::highlight(selected);
            if reading.cursor == (c, r) {
                name_style = name_style.add_modifier(Modifier::REVERSED);
            }
            let mut spans = vec![
                Span::styled(if selected { "☞" } else { " " }, theme::ink2()),
                Span::styled(format!("{:>2} ", entry.glyph), Style::new().fg(theme::cat_color(&entry.tag))),
                Span::styled(entry.name.clone(), name_style),
            ];
            if !entry.detail.is_empty() {
                spans.push(Span::styled(format!("  {}", entry.detail), Style::new().fg(theme::INK3)));
            }
            lines.push(Line::from(spans));
        }
        frame.render_widget(Paragraph::new(lines), col_area);
    }
}

fn view_apparatus(reading: &Reading, visible: usize, area: Rect, frame: &mut Frame) {
    let total = reading.chart.excerpts.len();
    let (any_style, all_style) = match reading.mode {
        Mode::Any => (Style::new().fg(theme::BG).bg(theme::INK2), theme::ink2()),
        Mode::All => (theme::ink2(), Style::new().fg(theme::BG).bg(theme::INK2)),
    };
    let line = Line::from(vec![
        Span::styled(" passages touching ", theme::apparatus()),
        Span::styled(" any ", any_style),
        Span::styled(" all ", all_style),
        Span::styled(" of the selection", theme::apparatus()),
        Span::styled(format!("   ·   {visible} of {total} passages"), theme::apparatus()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn view_commentary(
    reading: &Reading,
    visible: &[&astro::contract::Excerpt],
    area: Rect,
    frame: &mut Frame,
) {
    let [rubric_area, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(Paragraph::new(rubric_line("Commentary")), rubric_area);

    let mut lines: Vec<Line> = Vec::new();
    if visible.is_empty() {
        lines.push(Line::raw(""));
        lines.push(
            Line::from(Span::styled(
                if reading.chart.excerpts.is_empty() {
                    "no transcript passages were routed to this chart"
                } else {
                    "no passage touches the selection — c clears it"
                },
                theme::apparatus(),
            ))
            .alignment(Alignment::Center),
        );
    }
    // Manual wrapping keeps the folio gutter: continuation lines hang at the
    // quote's left edge instead of falling back to column zero.
    const GUTTER: usize = 11;
    let text_width = (body.width as usize).saturating_sub(GUTTER).max(16);
    for ex in visible {
        let folio = if ex.time.is_empty() { "—".to_string() } else { ex.time.clone() };
        for (i, row) in wrap(&format!("“{}”", ex.text), text_width).into_iter().enumerate() {
            let gutter = if i == 0 { format!("{folio:>9}  ") } else { " ".repeat(GUTTER) };
            lines.push(Line::from(vec![
                Span::styled(gutter, Style::new().fg(theme::INK3)),
                Span::styled(row, Style::new().fg(theme::INK)),
            ]));
        }
        let mut tag_spans = vec![Span::styled("           vide ", theme::apparatus())];
        for (i, tag) in ex.tags.iter().enumerate() {
            if i > 0 {
                tag_spans.push(Span::styled(" · ", Style::new().fg(theme::INK3)));
            }
            let name = reading.tag_names.get(tag).cloned().unwrap_or_else(|| tag.clone());
            tag_spans.push(Span::styled(name, Style::new().fg(theme::cat_color(tag))));
        }
        lines.push(Line::from(tag_spans));
        lines.push(Line::raw(""));
    }
    frame.render_widget(Paragraph::new(lines).scroll((reading.scroll, 0)), body);
}

/// Greedy word wrap to a column width (by char count — adequate for the
/// commentary's prose).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows = vec![String::new()];
    for word in text.split_whitespace() {
        let row = rows.last_mut().unwrap();
        let need = if row.is_empty() { 0 } else { 1 } + word.chars().count();
        if !row.is_empty() && row.chars().count() + need > width {
            rows.push(word.to_string());
        } else {
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(word);
        }
    }
    rows
}


// -------------------------------------------------------------- status ----


fn view_status(model: &Model, area: Rect, frame: &mut Frame) {
    let hints = match &model.screen {
        Screen::Form(_) => "",
        Screen::Reading(_) => {
            "arrows · move   space · toggle   a · any/all   c · clear   j/k · scroll   e · engrave html   b · back   q · quit"
        }
    };
    let status = match model.job {
        Some(Job::Transcribing(pct)) => format!("transcribing… {pct}%"),
        Some(Job::Computing) => "computing the figure…".to_string(),
        None => model.status.clone(),
    };
    let line = Line::from(vec![
        Span::styled(format!(" {status}"), theme::ink2().italic()),
        Span::raw("   "),
        Span::styled(hints, theme::hairline().italic()),
    ]);
    frame.render_widget(Paragraph::new(line).style(theme::base()), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testkit::reading_model;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_to_text(model: &Model, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| view(model, f)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn form_screen_renders() {
        let model = Model::default();
        let text = render_to_text(&model, 100, 32);
        assert!(text.contains("A S T R O"));
        assert!(text.contains("born on"));
        assert!(text.contains("☞"));
        println!("{text}");
    }

    #[test]
    fn reading_screen_renders() {
        let model = reading_model();
        let text = render_to_text(&model, 150, 45);
        for needle in [
            "Fig. I",
            "I n d e x",
            "E l e m e n t s",
            "C o m m e n t a r y",
            "planets",
            "9 of 9 passages",
            "vide",
            "☉",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in rendered TUI");
        }
        println!("{text}");
    }
}
