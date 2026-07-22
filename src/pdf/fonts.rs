//! Embedded fonts for the PDF rendition and the advance-width measurer the
//! layout engine wraps text with. Libre Baskerville (OFL 1.1) carries the
//! artifact's Baskerville voice onto paper for Latin-script readings; PT Serif
//! (OFL 1.1, ParaType) covers the same register for Cyrillic, since Libre
//! Baskerville has no Cyrillic glyphs. DejaVu Sans (Bitstream Vera license)
//! supplies the astrological glyphs for every locale. All live in
//! `assets/fonts/` and are subset by krilla on write.

use crate::i18n::Locale;
use krilla::text::Font;
use std::borrow::Cow;

static REGULAR: &[u8] = include_bytes!("../../assets/fonts/LibreBaskerville-Regular.ttf");
static ITALIC: &[u8] = include_bytes!("../../assets/fonts/LibreBaskerville-Italic.ttf");
// Cyrillic-capable serif for Russian (and future Cyrillic locales); Latin
// output keeps Libre Baskerville, so English PDFs stay byte-identical.
static REGULAR_CYRILLIC: &[u8] = include_bytes!("../../assets/fonts/PTSerif-Regular.ttf");
static ITALIC_CYRILLIC: &[u8] = include_bytes!("../../assets/fonts/PTSerif-Italic.ttf");
// Pre-subset to the astro glyphs (23 KB instead of 757 KB in every binary);
// regenerate from the full DejaVuSans.ttf with:
//   pyftsubset DejaVuSans.ttf --output-file=DejaVuSans-astro.ttf \
//     --no-layout-closure \
//     --unicodes="U+0020,U+002A,U+00B0,U+00B7,U+2217,U+25A1,U+25B3,U+2600-2653,U+2731,U+2736"
// The catalog-coverage test guards against over-cutting.
static SYMBOLS: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-astro.ttf");

#[derive(Clone, Copy, PartialEq)]
pub enum Face {
    Regular,
    Italic,
    Symbols,
}

/// Multi-char "glyphs" (the Ascendant's "AC", roman house labels) are words
/// set in the text face; single astrological glyphs live in the symbol font.
/// The one home for that rule — the wheel, the index, and the folio chips
/// all route through it.
pub fn glyph_face(glyph: &str) -> Face {
    if glyph.chars().count() > 1 { Face::Regular } else { Face::Symbols }
}

/// DejaVu lacks the sextile ⚹ (U+26B9); the six-spoked asterisk operator is
/// its faithful stand-in. Applied inside every measure and draw of the
/// symbol face — callers never substitute themselves.
pub(super) fn sym(text: &str) -> Cow<'_, str> {
    if text.contains('\u{26B9}') {
        Cow::Owned(text.replace('\u{26B9}', "\u{2217}"))
    } else {
        Cow::Borrowed(text)
    }
}

pub struct Fonts {
    regular: Font,
    italic: Font,
    symbols: Font,
    reg_face: ttf_parser::Face<'static>,
    ital_face: ttf_parser::Face<'static>,
    sym_face: ttf_parser::Face<'static>,
}

impl Fonts {
    /// Load the faces for a reading's locale: the body serif is chosen so its
    /// script covers the locale (Latin → Libre Baskerville, Cyrillic → PT
    /// Serif); the astrological symbol font is shared across locales.
    pub fn new(loc: Locale) -> Result<Fonts, String> {
        let (regular_bytes, italic_bytes) = match loc {
            Locale::Ru => (REGULAR_CYRILLIC, ITALIC_CYRILLIC),
            Locale::En => (REGULAR, ITALIC),
        };
        let font = |data: &'static [u8], name: &str| {
            Font::new(data.into(), 0).ok_or(format!("cannot load the embedded {name} font"))
        };
        let face = |data: &'static [u8], name: &str| {
            ttf_parser::Face::parse(data, 0)
                .map_err(|e| format!("cannot parse the embedded {name} font: {e}"))
        };
        Ok(Fonts {
            regular: font(regular_bytes, "regular")?,
            italic: font(italic_bytes, "italic")?,
            symbols: font(SYMBOLS, "symbol")?,
            reg_face: face(regular_bytes, "regular")?,
            ital_face: face(italic_bytes, "italic")?,
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
        let text: Cow<str> = if face == Face::Symbols { sym(text) } else { Cow::Borrowed(text) };
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

    /// Whether every char of `text` (after substitution) maps to a real
    /// glyph — the coverage tests assert this for the whole symbol catalog.
    #[cfg(test)]
    pub fn covers(&self, face: Face, text: &str) -> bool {
        let text: Cow<str> = if face == Face::Symbols { sym(text) } else { Cow::Borrowed(text) };
        text.chars().all(|c| self.face(face).glyph_index(c).is_some())
    }
}
