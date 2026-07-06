use std::collections::HashMap;
use std::fmt::Write;

use anyhow::Result;

use crate::anim::Animation;
use crate::theme::Theme;

use super::{RenderConfig, svg, text};
use svg::fmt;

/// Assemble an animated SVG: one absolutely-positioned group per keyframe,
/// toggled by per-frame CSS keyframes on opacity. `step-end` timing makes
/// each frame hold until the next stop and then jump, and adjacent frames
/// share the exact same formatted percentage for "off"/"on", so rounding
/// can never open a gap or an overlap. CSS animations inside <style> are
/// the one technique that reliably plays inside <img> embeds (GitHub
/// READMEs included).
pub fn render_animated(
    anim: &Animation,
    theme: &Theme,
    config: &RenderConfig,
    looping: bool,
) -> Result<String> {
    let l = svg::layout(anim.cols, anim.rows, config)?;
    let css = animation_css(anim, looping);

    let mut out = String::with_capacity(64 * 1024);
    svg::open_document(&mut out, &l, config);
    svg::style_block(&mut out, config, &css);
    svg::chrome_layer(&mut out, theme, config, &l, "");

    // Screens repeat almost every row from frame to frame (typing appends;
    // scrolling shifts), so each distinct row is defined once and frames
    // reference it by y offset — the difference between O(rows × frames)
    // and O(distinct rows) output.
    let mut defs = String::new();
    let mut row_ids: HashMap<String, usize> = HashMap::new();
    let mut frame_bodies = Vec::with_capacity(anim.frames.len());
    for frame in &anim.frames {
        let mut body = String::new();
        for (row, runs) in frame.screen.rows.iter().enumerate() {
            if runs.is_empty() {
                continue;
            }
            let mut markup = String::new();
            text::row_background_rects(&mut markup, runs, &l.m, l.origin_x, 0.0);
            text::row_text_runs(&mut markup, runs, &l.m, l.origin_x, l.m.baseline(0));
            let id = match row_ids.get(&markup) {
                Some(&id) => id,
                None => {
                    let id = row_ids.len();
                    let _ = write!(defs, r#"<g id="r{id}">{markup}</g>"#);
                    defs.push('\n');
                    row_ids.insert(markup, id);
                    id
                }
            };
            let _ = write!(
                body,
                r##"<use href="#r{id}" y="{}"/>"##,
                fmt(l.origin_y + row as f32 * l.m.line_h),
            );
        }
        if let Some((col, row)) = frame.cursor {
            // Soft cursor block. A cursor in the pending-wrap state
            // reports col == cols; keep the block on the grid.
            let col = col.min(anim.cols.saturating_sub(1));
            let _ = write!(
                body,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.55"/>"#,
                fmt(l.origin_x + col as f32 * l.m.cell_w),
                fmt(l.origin_y + row as f32 * l.m.line_h),
                fmt(l.m.cell_w),
                fmt(l.m.line_h),
                theme.foreground.hex(),
            );
        }
        frame_bodies.push(body);
    }

    out.push_str("<defs>\n");
    out.push_str(&defs);
    out.push_str("</defs>\n");
    let _ = write!(out, r#"<g font-size="{}">"#, fmt(l.m.font_size));
    out.push('\n');
    for (i, body) in frame_bodies.iter().enumerate() {
        let _ = write!(out, r#"<g class="f" id="f{i}">{body}</g>"#);
        out.push('\n');
    }
    out.push_str("</g>\n</svg>\n");
    Ok(out)
}

fn animation_css(anim: &Animation, looping: bool) -> String {
    let pct: Vec<String> = anim
        .frames
        .iter()
        .map(|f| fmt_pct(f.time / anim.duration * 100.0))
        .collect();

    let mut css = String::with_capacity(96 * anim.frames.len() + 128);
    let _ = write!(
        css,
        ".f{{opacity:0;animation-duration:{}s;animation-timing-function:step-end;animation-iteration-count:{}}}",
        fmt_secs(anim.duration),
        if looping { "infinite" } else { "1" },
    );
    if !looping {
        // Hold the final keyframe values after the single pass.
        css.push_str(".f{animation-fill-mode:forwards}");
    }
    for i in 0..anim.frames.len() {
        let _ = write!(css, "#f{i}{{animation-name:k{i}}}");
    }
    for (i, on) in pct.iter().enumerate() {
        let _ = write!(css, "@keyframes k{i}{{");
        if i == 0 {
            css.push_str("0%{opacity:1}");
        } else {
            let _ = write!(css, "0%{{opacity:0}}{on}%{{opacity:1}}");
        }
        if let Some(off) = pct.get(i + 1) {
            let _ = write!(css, "{off}%{{opacity:0}}");
        }
        css.push('}');
    }
    css
}

/// Keyframe percentage: 4 decimals (60 µs of a 60 s loop), no trailing
/// zeros.
fn fmt_pct(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Duration in seconds, millisecond precision.
fn fmt_secs(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{AnimOptions, build_frames};
    use crate::cast::{Event, EventData, Header};
    use crate::theme::builtin;

    #[test]
    fn pct_and_secs_format() {
        assert_eq!(fmt_pct(0.0), "0");
        assert_eq!(fmt_pct(8.10417), "8.1042");
        assert_eq!(fmt_pct(100.0), "100");
        assert_eq!(fmt_secs(1.5), "1.5");
        assert_eq!(fmt_secs(12.0), "12");
    }

    #[test]
    fn animated_document_structure() {
        let theme = builtin::load("dracula").unwrap();
        let header = Header {
            version: 2,
            width: 10,
            height: 3,
            timestamp: None,
            title: None,
            env: None,
            idle_time_limit: None,
            theme: None,
        };
        let events = [
            Event {
                time: 0.5,
                data: EventData::Output("hi".into()),
            },
            Event {
                time: 1.5,
                data: EventData::Output(" there".into()),
            },
        ];
        let anim = build_frames(
            &header,
            &events,
            &theme,
            &AnimOptions {
                idle_time_limit: None,
                speed: 1.0,
            },
        );
        let config = RenderConfig {
            font_size: 14.0,
            line_height: 1.2,
            padding: 16.0,
            margin: 24.0,
            chrome: crate::render::ChromeStyle::Macos,
            background: true,
            shadow: true,
            title: None,
            font_family: "monospace".into(),
            font_faces: None,
        };

        let svg = render_animated(&anim, &theme, &config, true).unwrap();
        assert_eq!(svg.matches(r#"<g class="f""#).count(), 3);
        assert!(svg.contains("animation-iteration-count:infinite"));
        assert!(svg.contains("@keyframes k0{0%{opacity:1}"));
        assert!(svg.contains("step-end"));
        // Frame 1 turns on at the same percentage frame 0 turns off.
        let dur = 1.5 + 1.5; // last event + trailing pause
        let p1 = fmt_pct(0.5 / dur * 100.0);
        assert!(svg.contains(&format!("@keyframes k1{{0%{{opacity:0}}{p1}%{{opacity:1}}")));
        assert!(svg.contains(&format!("0%{{opacity:1}}{p1}%{{opacity:0}}")));
        // Cursor block present.
        assert!(svg.contains(r#" opacity="0.55"/>"#));

        let once = render_animated(&anim, &theme, &config, false).unwrap();
        assert!(once.contains("animation-iteration-count:1"));
        assert!(once.contains("animation-fill-mode:forwards"));
    }
}
