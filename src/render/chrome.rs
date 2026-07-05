use std::fmt::Write;

use crate::theme::Theme;

use super::RenderConfig;
use super::svg::{fmt, xml_escape};

pub const TITLE_BAR_H: f32 = 44.0;
pub const CORNER_RADIUS: f32 = 10.0;
const LIGHT_RADIUS: f32 = 6.5;
const LIGHT_SPACING: f32 = 20.0;

/// Draw the window: shadowed rounded body, traffic lights, centered title.
/// (x, y) is the window's top-left; content starts TITLE_BAR_H below when
/// the chrome is enabled.
pub fn window(
    out: &mut String,
    theme: &Theme,
    config: &RenderConfig,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let filter = if config.shadow {
        r#" filter="url(#shadow)""#
    } else {
        ""
    };
    let _ = write!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}"{}/>"#,
        fmt(x),
        fmt(y),
        fmt(w),
        fmt(h),
        fmt(CORNER_RADIUS),
        theme.background.hex(),
        filter,
    );
    out.push('\n');

    if !config.window {
        return;
    }

    let cy = y + TITLE_BAR_H / 2.0;
    for (i, light) in theme.lights.iter().enumerate() {
        let _ = write!(
            out,
            r#"<circle cx="{}" cy="{}" r="{}" fill="{}"/>"#,
            fmt(x + LIGHT_SPACING * (i + 1) as f32),
            fmt(cy),
            fmt(LIGHT_RADIUS),
            light.hex(),
        );
    }
    out.push('\n');

    if let Some(title) = config.title.as_deref().filter(|t| !t.is_empty()) {
        let _ = write!(
            out,
            r#"<text x="{}" y="{}" text-anchor="middle" dominant-baseline="central" font-size="{}" fill="{}">{}</text>"#,
            fmt(x + w / 2.0),
            fmt(cy),
            fmt(config.font_size * 0.9),
            theme.title_fg.hex(),
            xml_escape(title),
        );
        out.push('\n');
    }
}

/// Layered window shadow: a soft ambient pass under a tight contact pass,
/// like a real macOS window. The spread (dy + 3·stdDeviation ≈ 23px) stays
/// inside the default 24px margin so the shadow fades out naturally instead
/// of clipping into a box at the SVG edge. The theme's shadow_opacity scales
/// both passes.
pub fn shadow_filter(out: &mut String, opacity: f32) {
    let _ = write!(
        out,
        concat!(
            r#"<defs><filter id="shadow" x="-25%" y="-25%" width="150%" height="160%">"#,
            r#"<feDropShadow in="SourceGraphic" dx="0" dy="5" stdDeviation="6" flood-opacity="{ambient}" result="s1"/>"#,
            r#"<feDropShadow in="s1" dx="0" dy="1" stdDeviation="1.5" flood-opacity="{contact}"/>"#,
            r#"</filter></defs>"#,
        ),
        ambient = fmt(opacity * 0.55),
        contact = fmt(opacity * 0.8),
    );
    out.push('\n');
}
