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

/// The single feDropShadow filter definition shared by the window body.
pub fn shadow_filter(out: &mut String, opacity: f32) {
    let _ = write!(
        out,
        r#"<defs><filter id="shadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="12" stdDeviation="16" flood-opacity="{}"/></filter></defs>"#,
        fmt(opacity),
    );
    out.push('\n');
}
