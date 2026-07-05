use anyhow::{Context, Result};

/// Font-derived layout metrics, all in SVG user units (px).
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub font_size: f32,
    /// Advance width of one terminal column.
    pub cell_w: f32,
    /// Height of one terminal row.
    pub line_h: f32,
    /// Extent above the baseline.
    pub ascent: f32,
    /// Extent below the baseline (positive).
    pub descent: f32,
    /// Distance below the baseline to the underline center.
    pub underline_offset: f32,
    pub underline_thickness: f32,
    /// Distance above the baseline to the strikeout center.
    pub strikeout_offset: f32,
}

/// Read metrics from a font face so SVG geometry matches the embedded font
/// exactly.
pub fn from_font(data: &[u8], font_size: f32, line_height: f32) -> Result<Metrics> {
    let face = ttf_parser::Face::parse(data, 0).context("failed to parse bundled font")?;
    let scale = font_size / face.units_per_em() as f32;

    let advance = face
        .glyph_index('M')
        .and_then(|glyph| face.glyph_hor_advance(glyph))
        .context("bundled font has no 'M' advance")? as f32;

    let underline = face.underline_metrics();
    let strikeout = face.strikeout_metrics();

    Ok(Metrics {
        font_size,
        cell_w: advance * scale,
        line_h: (font_size * line_height).ceil(),
        ascent: face.ascender() as f32 * scale,
        descent: -face.descender() as f32 * scale,
        underline_offset: underline.map_or(font_size * 0.1, |m| -m.position as f32 * scale),
        underline_thickness: underline
            .map_or(font_size * 0.07, |m| m.thickness as f32 * scale)
            .max(1.0),
        strikeout_offset: strikeout.map_or(font_size * 0.26, |m| m.position as f32 * scale),
    })
}

impl Metrics {
    /// Baseline y (relative to the content origin) for a given row: the
    /// ascent/descent box is vertically centered inside the line box.
    pub fn baseline(&self, row: usize) -> f32 {
        row as f32 * self.line_h + (self.line_h + self.ascent - self.descent) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::assets;

    #[test]
    fn jetbrains_mono_metrics() {
        let m = from_font(assets::regular(), 14.0, 1.2).unwrap();
        // JBM: upem 1000, advance 600 → cell 8.4px at 14px
        assert!((m.cell_w - 8.4).abs() < 0.01, "cell_w = {}", m.cell_w);
        assert_eq!(m.line_h, 17.0); // ceil(16.8)
        assert!(m.ascent > m.descent);
        assert!(m.underline_thickness >= 1.0);
    }
}
