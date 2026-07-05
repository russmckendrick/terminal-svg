use std::fmt::Write;

use anyhow::Result;

use crate::font::assets;
use crate::term::screen::Screen;
use crate::theme::Theme;

use super::{RenderConfig, chrome, metrics, text};

/// Assemble the complete SVG document.
pub fn render(screen: &Screen, theme: &Theme, config: &RenderConfig) -> Result<String> {
    let m = metrics::from_font(assets::regular(), config.font_size, config.line_height)?;

    let rows = screen.rows.len().max(1);
    let content_w = screen.cols as f32 * m.cell_w + 2.0 * config.padding;
    let content_h = rows as f32 * m.line_h + 2.0 * config.padding;
    let title_bar_h = if config.window {
        chrome::TITLE_BAR_H
    } else {
        0.0
    };
    let win_w = content_w;
    let win_h = title_bar_h + content_h;
    let total_w = win_w + 2.0 * config.margin;
    let total_h = win_h + 2.0 * config.margin;

    let origin_x = config.margin + config.padding;
    let origin_y = config.margin + title_bar_h + config.padding;

    let mut out = String::with_capacity(16 * 1024);
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{ff}" xml:space="preserve">"#,
        w = fmt(total_w),
        h = fmt(total_h),
        ff = xml_escape(&config.font_family),
    );
    out.push('\n');

    style_block(&mut out, config);
    if config.shadow {
        chrome::shadow_filter(&mut out, theme.shadow_opacity);
    }
    chrome::window(
        &mut out,
        theme,
        config,
        config.margin,
        config.margin,
        win_w,
        win_h,
    );

    text::background_rects(&mut out, screen, &m, origin_x, origin_y);
    let _ = write!(out, r#"<g font-size="{}">"#, fmt(m.font_size));
    out.push('\n');
    text::text_runs(&mut out, screen, &m, origin_x, origin_y);
    out.push_str("</g>\n</svg>\n");
    Ok(out)
}

fn style_block(out: &mut String, config: &RenderConfig) {
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
    out.push_str("</style>\n");
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
