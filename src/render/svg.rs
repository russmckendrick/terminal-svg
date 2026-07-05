use std::fmt::Write;

use anyhow::Result;

use crate::font::assets;
use crate::term::screen::Screen;
use crate::theme::Theme;

use super::{RenderConfig, chrome, metrics, text};

/// Document geometry shared by the static and animated renderers.
pub(super) struct Layout {
    pub m: metrics::Metrics,
    pub win_w: f32,
    pub win_h: f32,
    pub total_w: f32,
    pub total_h: f32,
    /// Top-left of the text grid.
    pub origin_x: f32,
    pub origin_y: f32,
}

pub(super) fn layout(cols: usize, rows: usize, config: &RenderConfig) -> Result<Layout> {
    let m = metrics::from_font(assets::regular(), config.font_size, config.line_height)?;
    let content_w = cols as f32 * m.cell_w + 2.0 * config.padding;
    let content_h = rows as f32 * m.line_h + 2.0 * config.padding;
    let title_bar_h = if config.window {
        chrome::TITLE_BAR_H
    } else {
        0.0
    };
    let win_w = content_w;
    let win_h = title_bar_h + content_h;
    Ok(Layout {
        m,
        win_w,
        win_h,
        total_w: win_w + 2.0 * config.margin,
        total_h: win_h + 2.0 * config.margin,
        origin_x: config.margin + config.padding,
        origin_y: config.margin + title_bar_h + config.padding,
    })
}

/// Assemble the complete SVG document.
pub fn render(screen: &Screen, theme: &Theme, config: &RenderConfig) -> Result<String> {
    let l = layout(screen.cols, screen.rows.len().max(1), config)?;

    let mut out = String::with_capacity(16 * 1024);
    open_document(&mut out, &l, config);
    style_block(&mut out, config, "");
    chrome_layer(&mut out, theme, config, &l);

    text::background_rects(&mut out, screen, &l.m, l.origin_x, l.origin_y);
    let _ = write!(out, r#"<g font-size="{}">"#, fmt(l.m.font_size));
    out.push('\n');
    text::text_runs(&mut out, screen, &l.m, l.origin_x, l.origin_y);
    out.push_str("</g>\n</svg>\n");
    Ok(out)
}

pub(super) fn open_document(out: &mut String, l: &Layout, config: &RenderConfig) {
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{ff}" xml:space="preserve">"#,
        w = fmt(l.total_w),
        h = fmt(l.total_h),
        ff = xml_escape(&config.font_family),
    );
    out.push('\n');
}

pub(super) fn style_block(out: &mut String, config: &RenderConfig, extra_css: &str) {
    out.push_str("<style>");
    if let Some(faces) = &config.font_faces {
        for (weight, data) in &faces.faces {
            let _ = write!(
                out,
                "@font-face{{font-family:'{family}';font-weight:{weight};src:url(data:font/woff2;base64,{data}) format('woff2')}}",
                family = faces.family,
            );
        }
    }
    out.push_str(".b{font-weight:700}.i{font-style:italic}");
    out.push_str(extra_css);
    out.push_str("</style>\n");
}

/// Shadow filter (when enabled) plus the window body and title bar.
pub(super) fn chrome_layer(out: &mut String, theme: &Theme, config: &RenderConfig, l: &Layout) {
    if config.shadow {
        chrome::shadow_filter(out, theme.shadow_opacity);
    }
    chrome::window(
        out,
        theme,
        config,
        config.margin,
        config.margin,
        l.win_w,
        l.win_h,
    );
}

/// Format a length: at most 2 decimal places, no trailing zeros.
pub fn fmt(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_trims_zeros() {
        assert_eq!(fmt(16.0), "16");
        assert_eq!(fmt(16.80), "16.8");
        assert_eq!(fmt(13.445), "13.44");
    }

    #[test]
    fn escapes_xml() {
        assert_eq!(xml_escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }
}
