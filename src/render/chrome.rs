use std::fmt::Write;

use crate::theme::{Rgb, Theme};

use super::RenderConfig;
use super::svg::{fmt, xml_escape};

/// Window chrome style. Chrome geometry is fixed-size like a real window:
/// it does not scale with the terminal font, so a large --font-size looks
/// like a screenshot of a large-font terminal, not a blown-up window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ChromeStyle {
    /// macOS: traffic lights on the left, centered title
    #[default]
    Macos,
    /// Windows PowerShell console: white title bar, square corners,
    /// caption buttons and scrollbar on the right
    Windows,
    /// Ubuntu GNOME Terminal: dark header bar with round buttons (orange
    /// close), centered title, and a menu bar
    Ubuntu,
    /// Bare panel without a title bar
    None,
}

// Ubuntu bar layout: header with the buttons/title, then the classic
// GNOME Terminal menu bar underneath.
const UBUNTU_HEADER_H: f32 = 34.0;
const UBUNTU_MENU_H: f32 = 26.0;

impl ChromeStyle {
    pub fn title_bar_h(self) -> f32 {
        match self {
            Self::Macos => 32.0,
            Self::Windows => 30.0,
            Self::Ubuntu => UBUNTU_HEADER_H + UBUNTU_MENU_H,
            Self::None => 0.0,
        }
    }

    pub fn corner_radius(self) -> f32 {
        match self {
            Self::Macos | Self::None => 10.0,
            // conhost windows are square.
            Self::Windows => 0.0,
            Self::Ubuntu => 6.0,
        }
    }

    /// Extra window width to the right of the text grid (the conhost
    /// scrollbar).
    pub fn gutter_w(self) -> f32 {
        match self {
            Self::Windows => 14.0,
            _ => 0.0,
        }
    }

    /// Minimum window width so the title-bar furniture never overflows a
    /// very narrow terminal.
    pub fn min_width(self) -> f32 {
        match self {
            Self::Macos => 80.0,
            Self::Windows => 200.0,
            Self::Ubuntu => 320.0,
            Self::None => 0.0,
        }
    }
}

/// Title text size: fixed, like the rest of the chrome.
const TITLE_SIZE: f32 = 12.0;

// macOS traffic lights: 12px discs with centers 20px apart, matching the
// real thing.
const LIGHT_RADIUS: f32 = 6.0;
const LIGHT_SPACING: f32 = 20.0;

// Authentic OS chrome colors, overridable via the theme's [chrome]
// bar_bg/bar_fg keys.
const WINDOWS_BAR_BG: Rgb = Rgb::new(0xff, 0xff, 0xff);
const WINDOWS_BAR_FG: Rgb = Rgb::new(0x00, 0x00, 0x00);
const UBUNTU_BAR_FG: Rgb = Rgb::new(0xdf, 0xdb, 0xd2);

/// Draw the window: body, then the title bar for the configured style.
/// (x, y) is the window's top-left; content starts title_bar_h() below.
/// `filter_id` references a shadow filter emitted by [`shadow_filter`];
/// `id_suffix` keeps internal defs unique across dual-theme variants.
#[allow(clippy::too_many_arguments)]
pub fn window(
    out: &mut String,
    theme: &Theme,
    config: &RenderConfig,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    filter_id: Option<&str>,
    id_suffix: &str,
) {
    let filter = filter_id
        .map(|id| format!(r#" filter="url(#{id})""#))
        .unwrap_or_default();
    let _ = write!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}"{}/>"#,
        fmt(x),
        fmt(y),
        fmt(w),
        fmt(h),
        fmt(config.chrome.corner_radius()),
        theme.background.hex(),
        filter,
    );
    out.push('\n');

    match config.chrome {
        ChromeStyle::Macos => macos_bar(out, theme, config, x, y, w),
        ChromeStyle::Windows => windows_bar(out, theme, config, x, y, w, h),
        ChromeStyle::Ubuntu => ubuntu_bar(out, theme, config, x, y, w, id_suffix),
        ChromeStyle::None => {}
    }
}

fn macos_bar(out: &mut String, theme: &Theme, config: &RenderConfig, x: f32, y: f32, w: f32) {
    let cy = y + ChromeStyle::Macos.title_bar_h() / 2.0;
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
    title_text(
        out,
        config,
        x + w / 2.0,
        cy,
        "middle",
        "",
        "-apple-system,'Segoe UI',sans-serif",
        &theme.title_fg.hex(),
    );
}

/// Classic Windows PowerShell console: white title bar with the icon and
/// left-aligned title, thin black caption glyphs, and a light scrollbar
/// gutter down the right of the console area.
fn windows_bar(
    out: &mut String,
    theme: &Theme,
    config: &RenderConfig,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let bar_h = ChromeStyle::Windows.title_bar_h();
    let cy = y + bar_h / 2.0;
    let bar_bg = theme.bar_bg.unwrap_or(WINDOWS_BAR_BG).hex();
    let bar_fg = theme.bar_fg.unwrap_or(WINDOWS_BAR_FG).hex();

    let _ = write!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
        fmt(x),
        fmt(y),
        fmt(w),
        fmt(bar_h),
        bar_bg,
    );

    // PowerShell icon: blue tile, a white ">" chevron and prompt
    // underscore.
    let (ix, iy) = (x + 8.0, cy - 8.0);
    let _ = write!(
        out,
        concat!(
            r##"<rect x="{ix}" y="{iy}" width="16" height="16" rx="2" fill="#2671be"/>"##,
            r##"<path d="M{cx} {cty}l3.5 3 -3.5 3" stroke="#fff" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##,
            r##"<line x1="{ux}" y1="{uy}" x2="{ux2}" y2="{uy}" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/>"##,
        ),
        ix = fmt(ix),
        iy = fmt(iy),
        cx = fmt(ix + 3.5),
        cty = fmt(iy + 5.0),
        ux = fmt(ix + 8.5),
        ux2 = fmt(ix + 12.5),
        uy = fmt(iy + 11.5),
    );

    // Caption glyphs, right-to-left: close, maximize, minimize — each in
    // the middle of a 46px-wide caption region.
    let close = x + w - 23.0;
    let max = x + w - 69.0;
    let min = x + w - 115.0;
    let _ = write!(
        out,
        concat!(
            r#"<line x1="{m1}" y1="{my}" x2="{m2}" y2="{my}" stroke="{fg}" stroke-width="1"/>"#,
            r#"<rect x="{rx}" y="{ry}" width="9" height="9" fill="none" stroke="{fg}" stroke-width="1"/>"#,
            r#"<path d="M{c1} {cy1}l9 9M{c2} {cy1}l-9 9" stroke="{fg}" stroke-width="1"/>"#,
        ),
        m1 = fmt(min - 4.5),
        m2 = fmt(min + 4.5),
        my = fmt(cy + 0.5),
        rx = fmt(max - 4.5),
        ry = fmt(cy - 4.5),
        c1 = fmt(close - 4.5),
        c2 = fmt(close + 4.5),
        cy1 = fmt(cy - 4.5),
        fg = bar_fg,
    );
    out.push('\n');

    // conhost scrollbar: light track with arrow caps and a thumb parked
    // at the top.
    let track_x = x + w - ChromeStyle::Windows.gutter_w();
    let track_y = y + bar_h;
    let track_h = h - bar_h;
    let acx = track_x + 7.0;
    let _ = write!(
        out,
        concat!(
            r##"<rect x="{tx}" y="{ty}" width="14" height="{th}" fill="#f0f0f0"/>"##,
            r##"<path d="M{a1x} {a1y}l3.5 -4 3.5 4Z" fill="#606060"/>"##,
            r##"<path d="M{a1x} {a2y}l3.5 4 3.5 -4Z" fill="#606060"/>"##,
            r##"<rect x="{bx}" y="{by}" width="8" height="{bh}" fill="#cdcdcd"/>"##,
        ),
        tx = fmt(track_x),
        ty = fmt(track_y),
        th = fmt(track_h),
        a1x = fmt(acx - 3.5),
        a1y = fmt(track_y + 9.0),
        a2y = fmt(y + h - 9.0),
        bx = fmt(track_x + 3.0),
        by = fmt(track_y + 14.0),
        bh = fmt((track_h - 28.0).max(10.0) * 0.3),
    );
    out.push('\n');

    title_text(
        out,
        config,
        x + 32.0,
        cy,
        "start",
        "",
        "'Segoe UI',sans-serif",
        &bar_fg,
    );
}

/// Ubuntu GNOME Terminal (Ambiance/Yaru): dark warm-grey header with a
/// centered bold title, round buttons with an orange close, and the
/// File/Edit/View menu bar underneath.
fn ubuntu_bar(
    out: &mut String,
    theme: &Theme,
    config: &RenderConfig,
    x: f32,
    y: f32,
    w: f32,
    id_suffix: &str,
) {
    let cy = y + UBUNTU_HEADER_H / 2.0;
    let bar_fg = theme.bar_fg.unwrap_or(UBUNTU_BAR_FG).hex();
    let r = ChromeStyle::Ubuntu.corner_radius();

    // Header: subtle vertical gradient like Ambiance, unless the theme
    // overrides it with a flat color. Rounded top corners only (the
    // clip below the header is covered by the menu bar).
    let grad_id = format!("ubar{id_suffix}");
    let header_fill = match theme.bar_bg {
        Some(bg) => bg.hex(),
        None => {
            let _ = write!(
                out,
                concat!(
                    r#"<defs><linearGradient id="{id}" x1="0" y1="0" x2="0" y2="1">"#,
                    r##"<stop offset="0" stop-color="#4c4641"/>"##,
                    r##"<stop offset="1" stop-color="#3a3531"/>"##,
                    r#"</linearGradient></defs>"#,
                ),
                id = grad_id,
            );
            format!("url(#{grad_id})")
        }
    };
    let _ = write!(
        out,
        concat!(
            r#"<path d="M{x} {yr}a{r} {r} 0 0 1 {r} -{r}h{iw}a{r} {r} 0 0 1 {r} {r}v{hh}h-{w}Z" fill="{fill}"/>"#,
            r##"<rect x="{x}" y="{my}" width="{w}" height="{mh}" fill="#3b3733"/>"##,
        ),
        x = fmt(x),
        yr = fmt(y + r),
        r = fmt(r),
        iw = fmt(w - 2.0 * r),
        hh = fmt(UBUNTU_HEADER_H - r),
        w = fmt(w),
        my = fmt(y + UBUNTU_HEADER_H),
        mh = fmt(UBUNTU_MENU_H),
        fill = header_fill,
    );
    out.push('\n');

    // Round buttons, right-to-left: orange close, then maximize and
    // minimize in recessed grey.
    let close = x + w - 26.0;
    let max = x + w - 52.0;
    let min = x + w - 78.0;
    for cx in [min, max] {
        let _ = write!(
            out,
            r##"<circle cx="{}" cy="{}" r="9" fill="{}" stroke="#5c5853" stroke-width="1"/>"##,
            fmt(cx),
            fmt(cy),
            theme.button_bg.hex(),
        );
    }
    let _ = write!(
        out,
        concat!(
            r##"<circle cx="{ccx}" cy="{cy}" r="9" fill="#f15d22" stroke="#c34113" stroke-width="1"/>"##,
            r#"<line x1="{m1}" y1="{my}" x2="{m2}" y2="{my}" stroke="{fg}" stroke-width="1.2"/>"#,
            r#"<rect x="{rx}" y="{ry}" width="7" height="7" fill="none" stroke="{fg}" stroke-width="1.2"/>"#,
            r##"<path d="M{c1} {cy1}l6 6M{c2} {cy1}l-6 6" stroke="#fff" stroke-width="1.2" stroke-linecap="round"/>"##,
        ),
        ccx = fmt(close),
        cy = fmt(cy),
        m1 = fmt(min - 3.5),
        m2 = fmt(min + 3.5),
        my = fmt(cy + 2.5),
        rx = fmt(max - 3.5),
        ry = fmt(cy - 3.5),
        c1 = fmt(close - 3.0),
        c2 = fmt(close + 3.0),
        cy1 = fmt(cy - 3.0),
        fg = bar_fg,
    );
    out.push('\n');

    // Menu bar items at fixed offsets (sans text metrics vary, so the
    // spacing is generous).
    let menu_cy = y + UBUNTU_HEADER_H + UBUNTU_MENU_H / 2.0;
    for (dx, item) in [
        (12.0, "File"),
        (50.0, "Edit"),
        (88.0, "View"),
        (128.0, "Search"),
        (178.0, "Terminal"),
        (240.0, "Help"),
    ] {
        let _ = write!(
            out,
            r##"<text x="{}" y="{}" dominant-baseline="central" font-size="12" font-family="Ubuntu,'DejaVu Sans',sans-serif" fill="#d5d0cb">{item}</text>"##,
            fmt(x + dx),
            fmt(menu_cy),
        );
    }
    out.push('\n');

    title_text(
        out,
        config,
        x + w / 2.0,
        cy,
        "middle",
        "b",
        "Ubuntu,'DejaVu Sans',sans-serif",
        &bar_fg,
    );
}

#[allow(clippy::too_many_arguments)]
fn title_text(
    out: &mut String,
    config: &RenderConfig,
    x: f32,
    cy: f32,
    anchor: &str,
    class: &str,
    font: &str,
    fill: &str,
) {
    if let Some(title) = config.title.as_deref().filter(|t| !t.is_empty()) {
        let class = if class.is_empty() {
            String::new()
        } else {
            format!(r#" class="{class}""#)
        };
        let _ = write!(
            out,
            r#"<text x="{}" y="{}" text-anchor="{anchor}" dominant-baseline="central" font-size="{}" font-family="{font}"{class} fill="{fill}">{}</text>"#,
            fmt(x),
            fmt(cy),
            fmt(TITLE_SIZE),
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
pub fn shadow_filter(out: &mut String, opacity: f32, id: &str) {
    let _ = write!(
        out,
        concat!(
            r#"<defs><filter id="{id}" x="-25%" y="-25%" width="150%" height="160%">"#,
            r#"<feDropShadow in="SourceGraphic" dx="0" dy="5" stdDeviation="6" flood-opacity="{ambient}" result="s1"/>"#,
            r#"<feDropShadow in="s1" dx="0" dy="1" stdDeviation="1.5" flood-opacity="{contact}"/>"#,
            r#"</filter></defs>"#,
        ),
        id = id,
        ambient = fmt(opacity * 0.55),
        contact = fmt(opacity * 0.8),
    );
    out.push('\n');
}
