use std::fmt::Write;

use crate::term::screen::{PenColor, Screen, StyledRun};
use crate::theme::{Rgb, Theme};

use super::metrics::Metrics;
use super::svg::{fmt, xml_escape};
use super::{FillMode, palette_class};

/// The run's foreground as concrete RGB under one theme, faint applied.
fn resolved_fg(run: &StyledRun, theme: &Theme) -> Rgb {
    let fg = run.fg.resolve(theme);
    if run.faint {
        // Dim by blending halfway toward the effective background.
        let bg = run.bg.map_or(theme.background, |b| b.resolve(theme));
        fg.blend(bg, 0.5)
    } else {
        fg
    }
}

/// Emit the background-rect layer: adjacent same-bg cells were already merged
/// into runs, so each run with a non-default bg becomes one rect. Rects are
/// exactly line_h tall so rows tile without seams.
pub(super) fn background_rects(
    out: &mut String,
    screen: &Screen,
    m: &Metrics,
    origin_x: f32,
    origin_y: f32,
    mode: &FillMode,
) {
    for (row, runs) in screen.rows.iter().enumerate() {
        row_background_rects(
            out,
            runs,
            m,
            origin_x,
            origin_y + row as f32 * m.line_h,
            mode,
        );
        if !runs.is_empty() {
            out.push('\n');
        }
    }
}

/// One row's background rects, with `y_top` as the row's top edge.
pub(super) fn row_background_rects(
    out: &mut String,
    runs: &[StyledRun],
    m: &Metrics,
    origin_x: f32,
    y_top: f32,
    mode: &FillMode,
) {
    for run in runs {
        let Some(bg) = run.bg else { continue };
        let fill = match mode {
            FillMode::Hex(theme) => format!(r#"fill="{}""#, bg.resolve(theme).hex()),
            FillMode::Class => match bg {
                PenColor::Rgb(c) => format!(r#"fill="{}""#, c.hex()),
                classed => format!(r#"class="{}""#, palette_class(classed).expect("not Rgb")),
            },
        };
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" {fill}/>"#,
            fmt(origin_x + run.col as f32 * m.cell_w),
            fmt(y_top),
            fmt(run.width as f32 * m.cell_w),
            fmt(m.line_h),
        );
    }
}

/// Emit the text layer: one <text> element per styled run, plus drawn
/// under/strikethrough lines (CSS text-decoration is unreliable across
/// SVG renderers).
pub(super) fn text_runs(
    out: &mut String,
    screen: &Screen,
    m: &Metrics,
    origin_x: f32,
    origin_y: f32,
    mode: &FillMode,
) {
    for (row, runs) in screen.rows.iter().enumerate() {
        let baseline = origin_y + m.baseline(row);
        row_text_runs(out, runs, m, origin_x, baseline, mode);
        if !runs.is_empty() {
            out.push('\n');
        }
    }
}

/// The attributes after x/y on a run's <text> elements: fill and/or class
/// (bold/italic ride along), plus faint dimming in class mode.
fn text_attrs(run: &StyledRun, mode: &FillMode) -> String {
    let style_class = match (run.bold, run.italic) {
        (false, false) => None,
        (true, false) => Some("b"),
        (false, true) => Some("i"),
        (true, true) => Some("b i"),
    };
    match mode {
        FillMode::Hex(theme) => {
            let class_attr = style_class.map_or(String::new(), |c| format!(r#" class="{c}""#));
            format!(r#" fill="{}"{class_attr}"#, resolved_fg(run, theme).hex())
        }
        FillMode::Class => {
            let mut attrs = String::new();
            let mut classes = Vec::new();
            match run.fg {
                PenColor::Rgb(c) => {
                    let _ = write!(attrs, r#" fill="{}""#, c.hex());
                }
                classed => classes.push(palette_class(classed).expect("not Rgb")),
            }
            if let Some(c) = style_class {
                classes.push(c.to_string());
            }
            if !classes.is_empty() {
                let _ = write!(attrs, r#" class="{}""#, classes.join(" "));
            }
            if run.faint {
                // The palette is switchable, so faint can't pre-blend;
                // half opacity over the effective background is the same
                // dimming.
                attrs.push_str(r#" fill-opacity="0.5""#);
            }
            attrs
        }
    }
}

/// The color attribute for a run's decoration <line> elements, which
/// stroke rather than fill (the palette CSS pairs `line.cN{stroke:…}`).
fn line_attrs(run: &StyledRun, mode: &FillMode) -> String {
    match mode {
        FillMode::Hex(theme) => format!(r#"stroke="{}""#, resolved_fg(run, theme).hex()),
        FillMode::Class => {
            let mut attrs = match run.fg {
                PenColor::Rgb(c) => format!(r#"stroke="{}""#, c.hex()),
                classed => format!(r#"class="{}""#, palette_class(classed).expect("not Rgb")),
            };
            if run.faint {
                attrs.push_str(r#" stroke-opacity="0.5""#);
            }
            attrs
        }
    }
}

/// One row's text runs and decoration lines, at an absolute baseline y.
pub(super) fn row_text_runs(
    out: &mut String,
    runs: &[StyledRun],
    m: &Metrics,
    origin_x: f32,
    baseline: f32,
    mode: &FillMode,
) {
    for run in runs {
        let x = origin_x + run.col as f32 * m.cell_w;

        // XML whitespace collapsing is renderer-dependent (Chrome ignores
        // xml:space), so space glyphs are never emitted: each maximal
        // space-free segment becomes its own explicitly-positioned
        // <text>. Backgrounds and decorations are separate rects/lines,
        // so spaces carry no other visual information.
        let attrs = text_attrs(run, mode);
        for (offset, segment) in space_free_segments(&run.text) {
            // Wide runs are single-char, so offset is 0 and per-char
            // column width never matters within a segment.
            let _ = write!(
                out,
                r#"<text x="{}" y="{}"{attrs}>{}</text>"#,
                fmt(x + offset as f32 * m.cell_w),
                fmt(baseline),
                xml_escape(segment),
            );
        }

        let run_w = run.width as f32 * m.cell_w;
        if run.underline || run.strikethrough {
            let color_attr = line_attrs(run, mode);
            if run.underline {
                decoration_line(
                    out,
                    x,
                    run_w,
                    baseline + m.underline_offset,
                    m.underline_thickness,
                    &color_attr,
                );
            }
            if run.strikethrough {
                decoration_line(
                    out,
                    x,
                    run_w,
                    baseline - m.strikeout_offset,
                    m.underline_thickness,
                    &color_attr,
                );
            }
        }
    }
}

/// Maximal space-free segments of a run's text with their char offsets.
fn space_free_segments(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split(' ')
        .scan(0usize, |col, segment| {
            let start = *col;
            *col += segment.chars().count() + 1;
            Some((start, segment))
        })
        .filter(|(_, segment)| !segment.is_empty())
}

fn decoration_line(out: &mut String, x: f32, width: f32, y: f32, thickness: f32, color_attr: &str) {
    let _ = write!(
        out,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" {color_attr} stroke-width="{}"/>"#,
        fmt(x),
        fmt(y),
        fmt(x + width),
        fmt(y),
        fmt(thickness),
    );
}
