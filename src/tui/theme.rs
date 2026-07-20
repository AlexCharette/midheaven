//! The engraved star-atlas voice, translated to terminal cells: the
//! artifact's validated palette as RGB, italics and dim for apparatus text,
//! letterspaced rubrics, category colors keyed by tag prefix.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub const BG: Color = Color::Rgb(0x13, 0x17, 0x35); // midnight indigo
pub const INK: Color = Color::Rgb(0xe9, 0xe4, 0xd3);
pub const INK2: Color = Color::Rgb(0xbc, 0xb6, 0xa0);
pub const INK3: Color = Color::Rgb(0x9a, 0x95, 0x7f);
/// The artifact's hairline/line rgba tones, pre-blended onto the indigo.
pub const HAIRLINE: Color = Color::Rgb(0x53, 0x55, 0x64);
pub const LINE: Color = Color::Rgb(0x35, 0x38, 0x4e);

pub const BRASS: Color = Color::Rgb(0xc4, 0x9a, 0x30); // planets
pub const VERDIGRIS: Color = Color::Rgb(0x35, 0xab, 0x7c); // signs
pub const STEEL: Color = Color::Rgb(0x7b, 0xa0, 0xe0); // houses
pub const OXBLOOD: Color = Color::Rgb(0xc0, 0x54, 0x68); // aspects

pub fn base() -> Style {
    Style::new().fg(INK).bg(BG)
}

pub fn ink2() -> Style {
    Style::new().fg(INK2)
}

pub fn apparatus() -> Style {
    Style::new().fg(INK3).add_modifier(Modifier::ITALIC)
}

pub fn hairline() -> Style {
    Style::new().fg(HAIRLINE)
}

pub fn error() -> Style {
    Style::new().fg(OXBLOOD).add_modifier(Modifier::ITALIC)
}

/// What "focused / chosen" looks like, everywhere: bright ink, underlined;
/// inactive items sit in ink-2.
pub fn highlight(active: bool) -> Style {
    if active {
        Style::new().fg(INK).add_modifier(Modifier::UNDERLINED)
    } else {
        Style::new().fg(INK2)
    }
}

/// The manicule marks the active/selected row; its blank twin keeps columns
/// aligned.
pub fn marker(active: bool) -> &'static str {
    if active { "☞ " } else { "  " }
}

/// Category color for a tag-id (`planet:sun` → brass, …).
pub fn cat_color(tag: &str) -> Color {
    match tag.split(':').next().unwrap_or("") {
        "planet" => BRASS,
        "sign" => VERDIGRIS,
        "house" => STEEL,
        "aspect" => OXBLOOD,
        _ => INK2,
    }
}

/// "Index of Elements" → "I n d e x   o f   E l e m e n t s" — the terminal
/// stand-in for the artifact's letterspaced small caps.
pub fn letterspace(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 {
            out.push(' ');
            if ch == ' ' {
                out.push(' ');
            }
        }
        out.push(ch);
    }
    out
}

/// A rubric span: letterspaced, ink-2 — flanking rules come from the layout.
pub fn rubric(text: &str) -> Span<'static> {
    Span::styled(letterspace(text), Style::new().fg(INK2))
}
