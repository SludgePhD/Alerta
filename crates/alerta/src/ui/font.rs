use ab_glyph::{Font as _, Glyph, OutlinedGlyph, PxScaleFont, ScaleFont, point};
use raqote::DrawTarget;

use crate::ui::{Rgb, rgb};

const FALLBACK_FONT: &[u8] = include_bytes!("../../../../3rdparty/fonts/Cantarell-Regular.ttf");

pub(crate) struct Font {
    font: PxScaleFont<ab_glyph::FontRef<'static>>,
}

const FONT_SIZE: f32 = 18.0;

impl Font {
    /// Loads the font to use for the dialog contents.
    pub(crate) fn load() -> Self {
        // TODO: invoke `fc-match` to find a system font
        let inner = ab_glyph::FontRef::try_from_slice(FALLBACK_FONT).unwrap();

        Self {
            font: inner.into_scaled(FONT_SIZE),
        }
    }

    /// Returns a [`Renderer`] for rendering `text`.
    ///
    /// By default, `text` will be drawn in white and without soft wrapping.
    pub(crate) fn render<'a>(&'a self, text: &'a str) -> Renderer<'a> {
        Renderer {
            font: self,
            text,
            color: rgb(255, 255, 255),
            max_width: f32::MAX,
        }
    }
}

pub(crate) struct Renderer<'a> {
    font: &'a Font,
    text: &'a str,
    color: Rgb,
    max_width: f32,
}

impl<'a> Renderer<'a> {
    pub(crate) fn with_max_width(self, max_width: f32) -> Self {
        Self { max_width, ..self }
    }

    pub(crate) fn with_color(self, color: Rgb) -> Self {
        Self { color, ..self }
    }

    pub(crate) fn finish(self) -> DrawTarget {
        let glyphs = self.layout();

        let bounds = glyphs
            .iter()
            .map(|g| g.px_bounds())
            .reduce(|mut sum, next| {
                sum.min.x = f32::min(sum.min.x, next.min.x);
                sum.min.y = f32::min(sum.min.y, next.min.y);

                sum.max.x = f32::max(sum.max.x, next.max.x);
                sum.max.y = f32::max(sum.max.y, next.max.y);

                sum
            })
            .unwrap_or_default();

        let width = bounds.width() as u16;
        let height = bounds.height() as u16;
        let mut target = DrawTarget::new(width.into(), height.into());
        let pixels = target.get_data_mut();

        for g in glyphs {
            let glyph_bounds = g.px_bounds();
            let offset = glyph_bounds.min - bounds.min;
            let (off_x, off_y) = (offset.x as u32, offset.y as u32);
            g.draw(|x, y, c| {
                let idx = (off_y + y) * width as u32 + off_x + x;
                let Some(pix) = pixels.get_mut(idx as usize) else {
                    return;
                };

                // `DrawTarget` expects pre-multiplied alpha.
                let Rgb(r, g, b) = self.color;
                let a = (c * 255.0).round() as u32;
                let r = r as u32 * a / 255;
                let g = g as u32 * a / 255;
                let b = b as u32 * a / 255;
                *pix = (a << 24) | (r << 16) | (g << 8) | b;
            });
        }

        target
    }

    /// Calculates the text layout and computes glyph outlines.
    ///
    /// This will respect hard line breaks (`\n`) and attempt to perform soft wrapping when a line
    /// exceeds the configured max width.
    /// Soft-wrapping may fail if the rendered text contains no (or insufficient) permissible line
    /// break opportunities, in which case the text will exceed the intended width.
    fn layout(&self) -> Vec<OutlinedGlyph> {
        let mut glyphs: Vec<Glyph> = Vec::new();

        let mut y = 0.0;
        for line in self.text.lines() {
            let mut x = 0.0;

            let mut last_softbreak: Option<usize> = None;

            // ID of the last glyph we placed; used to apply kerning.
            let mut last = None;

            for c in line.chars() {
                let mut glyph = self.font.font.scaled_glyph(c);
                if let Some(last) = last {
                    x += self.font.font.kern(last, glyph.id);
                }
                glyph.position = point(x, y);
                last = Some(glyph.id);

                x += self.font.font.h_advance(glyph.id);

                if c == ' ' || c == ZWSP {
                    last_softbreak = Some(glyphs.len());
                } else {
                    glyphs.push(glyph);

                    if x > self.max_width
                        && let Some(i) = last_softbreak
                    {
                        // Out of space on this line. Perform a soft line break.
                        // Glyph at index `i` and later will be moved to the next line.
                        y += self.font.font.height() + self.font.font.line_gap();
                        let x_diff = glyphs.get(i).map(|g| g.position.x).unwrap_or(0.0);
                        for glyph in &mut glyphs[i..] {
                            glyph.position.x -= x_diff;
                            glyph.position.y = y;
                        }
                        x -= x_diff;

                        last_softbreak = None;
                    }
                }
            }
            y += self.font.font.height() + self.font.font.line_gap();
        }
        glyphs
            .into_iter()
            .filter_map(|g| self.font.font.outline_glyph(g))
            .collect()
    }
}

const ZWSP: char = '\u{200b}';
