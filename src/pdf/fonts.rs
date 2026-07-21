//! Embedded fonts for the PDF rendition and the advance-width measurer the
//! layout engine wraps text with. Libre Baskerville (OFL 1.1) carries the
//! artifact's Baskerville voice onto paper; DejaVu Sans (Bitstream Vera
//! license) supplies the astrological glyphs. Both live in `assets/fonts/`
//! and are subset by krilla on write.

use krilla::text::Font;

static REGULAR: &[u8] = include_bytes!("../../assets/fonts/LibreBaskerville-Regular.ttf");
static ITALIC: &[u8] = include_bytes!("../../assets/fonts/LibreBaskerville-Italic.ttf");
static SYMBOLS: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");

#[derive(Clone, Copy, PartialEq)]
pub enum Face {
    Regular,
    Italic,
    Symbols,
}

pub struct Fonts {
    regular: Font,
    italic: Font,
    symbols: Font,
    reg_face: ttf_parser::Face<'static>,
    ital_face: ttf_parser::Face<'static>,
    sym_face: ttf_parser::Face<'static>,
}

/// DejaVu lacks the sextile ⚹ (U+26B9); the six-spoked asterisk operator is
/// its faithful stand-in. Every symbol string is drawn and measured through
/// this substitution.
pub fn sym(text: &str) -> String {
    text.replace('\u{26B9}', "\u{2217}")
}

impl Fonts {
    pub fn new() -> Result<Fonts, String> {
        let font = |data: &'static [u8], name: &str| {
            Font::new(data.into(), 0).ok_or(format!("cannot load the embedded {name} font"))
        };
        let face = |data: &'static [u8], name: &str| {
            ttf_parser::Face::parse(data, 0)
                .map_err(|e| format!("cannot parse the embedded {name} font: {e}"))
        };
        Ok(Fonts {
            regular: font(REGULAR, "regular")?,
            italic: font(ITALIC, "italic")?,
            symbols: font(SYMBOLS, "symbol")?,
            reg_face: face(REGULAR, "regular")?,
            ital_face: face(ITALIC, "italic")?,
            sym_face: face(SYMBOLS, "symbol")?,
        })
    }

    pub fn font(&self, face: Face) -> Font {
        match face {
            Face::Regular => self.regular.clone(),
            Face::Italic => self.italic.clone(),
            Face::Symbols => self.symbols.clone(),
        }
    }

    fn face(&self, face: Face) -> &ttf_parser::Face<'static> {
        match face {
            Face::Regular => &self.reg_face,
            Face::Italic => &self.ital_face,
            Face::Symbols => &self.sym_face,
        }
    }

    /// Advance width of `text` at `size` points (plus `tracking` points per
    /// gap). Kerning is ignored — a slight overestimate, harmless for
    /// ragged-right wrapping.
    pub fn width(&self, face: Face, size: f32, tracking: f32, text: &str) -> f32 {
        let f = self.face(face);
        let upem = f32::from(f.units_per_em());
        let mut units = 0.0f32;
        let mut chars = 0usize;
        for c in text.chars() {
            chars += 1;
            units += f
                .glyph_index(c)
                .and_then(|g| f.glyph_hor_advance(g))
                .map(f32::from)
                .unwrap_or(upem * 0.6);
        }
        units / upem * size + tracking * chars.saturating_sub(1) as f32
    }

    /// Whether every char of `text` maps to a real glyph — the coverage
    /// tests assert this for the whole symbol catalog.
    #[cfg(test)]
    pub fn covers(&self, face: Face, text: &str) -> bool {
        text.chars().all(|c| self.face(face).glyph_index(c).is_some())
    }
}
