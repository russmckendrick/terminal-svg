use std::fmt::Write;

use crate::term::screen::Screen;

use super::metrics::Metrics;
use super::svg::{fmt, xml_escape};

/// Emit the background-rect layer: adjacent same-bg cells were already merged
/// into runs, so each run with a non-default bg becomes one rect. Rects are
/// exactly line_h tall so rows tile without seams.
pub fn background_rects(
    out: &mut String,
    screen: &Screen,
    m: &Metrics,
    origin_x: f32,
    origin_y: f32,
) {
    for (row, runs) in screen.rows.iter().enumerate() {
        for run in runs {
            let Some(bg) = run.bg else { continue };
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                fmt(origin_x + run.col as f32 * m.cell_w),
                fmt(origin_y + row as f32 * m.line_h),
                fmt(run.width as f32 * m.cell_w),
                fmt(m.line_h),
                bg.hex(),
            );
        }
        if !runs.is_empty() {
            out.push('\n');
        }
    }
}

/// Emit the text layer: one <text> element per styled run, plus drawn
/// under/strikethrough lines (CSS text-decoration is unreliable across
/// SVG renderers).
pub fn text_runs(out: &mut String, screen: &Screen, m: &Metrics, origin_x: f32, origin_y: f32) {
    for (row, runs) in screen.rows.iter().enumerate() {
        let baseline = origin_y + m.baseline(row);
        for run in runs {
            let x = origin_x + run.col as f32 * m.cell_w;

            // XML whitespace collapsing is renderer-dependent (Chrome ignores
            // xml:space), so space glyphs are never emitted: each maximal
            // space-free segment becomes its own explicitly-positioned
            // <text>. Backgrounds and decorations are separate rects/lines,
            // so spaces carry no other visual information.
            let class_attr = match (run.bold, run.italic) {
                (false, false) => "",
                (true, false) => r#" class="b""#,
                (false, true) => r#" class="i""#,
                (true, true) => r#" class="b i""#,
            };
            for (offset, segment) in space_free_segments(&run.text) {
                // Wide runs are single-char, so offset is 0 and per-char
                // column width never matters within a segment.
                let _ = write!(
                    out,
                    r#"<text x="{}" y="{}" fill="{}"{}>{}</text>"#,
                    fmt(x + offset as f32 * m.cell_w),
                    fmt(baseline),
                    run.fg.hex(),
                    class_attr,
                    xml_escape(segment),
                );
            }

            let run_w = run.width as f32 * m.cell_w;
            if run.underline {
                decoration_line(
                    out,
                    x,
                    run_w,
                    baseline + m.underline_offset,
                    m.underline_thickness,
                    &run.fg.hex(),
                );
            }
            if run.strikethrough {
                decoration_line(
                    out,
                    x,
                    run_w,
                    baseline - m.strikeout_offset,
                    m.underline_thickness,
                    &run.fg.hex(),
                );
            }
        }
        if !runs.is_empty() {
            out.push('\n');
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

fn decoration_line(out: &mut String, x: f32, width: f32, y: f32, thickness: f32, color: &str) {
    let _ = write!(
        out,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
        fmt(x),
        fmt(y),
        fmt(x + width),
        fmt(y),
        color,
        fmt(thickness),
    );
}
