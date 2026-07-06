pub mod screen;

use crate::theme::{Rgb, xterm_indexed};
use screen::{PenColor, Screen, StyledRun};

const SCROLLBACK_LIMIT: usize = 100_000;

/// Feed captured bytes through a virtual terminal and return the resolved
/// final screen (scrollback + visible view), ready for rendering.
pub fn interpret(bytes: &[u8], cols: usize, rows: usize) -> Screen {
    let mut vt = avt::Vt::builder()
        .size(cols, rows)
        .scrollback_limit(SCROLLBACK_LIMIT)
        .build();
    vt.feed_str(&normalize_newlines(&String::from_utf8_lossy(bytes)));

    let mut out: Vec<Vec<StyledRun>> = vt.lines().map(runs_for_line).collect();
    while out.last().is_some_and(|row| row.is_empty()) {
        out.pop();
    }
    Screen { cols, rows: out }
}

/// Every window title set via OSC 0/2 in the stream, in order. avt
/// discards OSC payloads, so titles are scanned from the raw output here.
pub fn osc_titles(data: &str) -> Vec<String> {
    let mut titles = Vec::new();
    let mut rest = data;
    while let Some(start) = rest.find("\x1b]") {
        rest = &rest[start + 2..];
        let Some(body) = rest.strip_prefix("0;").or_else(|| rest.strip_prefix("2;")) else {
            continue;
        };
        // Terminated by BEL or ST (ESC \); an unterminated sequence ends
        // the scan.
        let Some(end) = body.find(['\x07', '\x1b']) else {
            break;
        };
        let t = &body[..end];
        if !t.is_empty() {
            titles.push(t.to_string());
        }
        rest = &body[end..];
    }
    titles
}

/// Incremental interpreter for animated replays: feed recorded output
/// chunks one at a time and snapshot the visible screen between them.
pub struct Interpreter {
    vt: avt::Vt,
}

impl Interpreter {
    pub fn new(cols: usize, rows: usize) -> Self {
        // Replays render the viewport only, so no scrollback is kept.
        let vt = avt::Vt::builder()
            .size(cols, rows)
            .scrollback_limit(0)
            .build();
        Self { vt }
    }

    /// Feed raw recorded output. Unlike `interpret`, newlines are not
    /// normalized: asciicast data came off a PTY, where ONLCR already
    /// produced `\r\n`.
    pub fn feed(&mut self, data: &str) {
        self.vt.feed_str(data);
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.vt.resize(cols, rows);
    }

    /// Snapshot the visible screen. Trailing blank rows are kept — an
    /// animation canvas has a fixed height.
    pub fn snapshot(&self) -> Screen {
        Screen {
            cols: self.vt.size().0,
            rows: self.vt.view().map(runs_for_line).collect(),
        }
    }

    /// Cursor grid position, when the cursor is visible.
    pub fn cursor(&self) -> Option<(usize, usize)> {
        let cursor = self.vt.cursor();
        cursor.visible.then_some((cursor.col, cursor.row))
    }
}

/// A VT treats LF strictly as "move down" — the tty driver's ONLCR is what
/// turns program `\n` into `\r\n` on a real terminal. PTY captures already
/// contain `\r\n`; piped/file input has bare `\n`, so emulate the line
/// discipline. Idempotent for input that is already normalized.
fn normalize_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev = '\0';
    for c in s.chars() {
        if c == '\n' && prev != '\r' {
            out.push('\r');
        }
        out.push(c);
        prev = c;
    }
    out
}

/// Resolved visual attributes of a cell: inverse swapped, colors kept
/// symbolic (faint dims at render time). Blink is rendered static.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Attrs {
    fg: PenColor,
    bg: Option<PenColor>,
    faint: bool,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

fn symbolic(color: avt::Color) -> PenColor {
    match color {
        avt::Color::Indexed(i) if i < 16 => PenColor::Indexed(i),
        avt::Color::Indexed(i) => PenColor::Rgb(xterm_indexed(i)),
        avt::Color::RGB(c) => PenColor::Rgb(Rgb::new(c.r, c.g, c.b)),
    }
}

fn resolve(pen: &avt::Pen) -> Attrs {
    let mut fg = pen.foreground().map_or(PenColor::DefaultFg, symbolic);
    let mut bg = pen.background().map(symbolic);

    if pen.is_inverse() {
        let new_bg = fg;
        fg = bg.unwrap_or(PenColor::DefaultBg);
        bg = Some(new_bg);
    }

    Attrs {
        fg,
        bg,
        faint: pen.is_faint(),
        bold: pen.is_bold(),
        italic: pen.is_italic(),
        underline: pen.is_underline(),
        strikethrough: pen.is_strikethrough(),
    }
}

fn runs_for_line(line: &avt::Line) -> Vec<StyledRun> {
    let mut runs: Vec<StyledRun> = Vec::new();
    let mut col = 0usize;

    for cell in line.cells() {
        let cell_width = cell.width() as usize;
        if cell_width == 0 {
            // Continuation cell of a wide character; already accounted for.
            continue;
        }
        let attrs = resolve(cell.pen());
        let wide = cell_width == 2;

        let mergeable = !wide
            && runs.last().is_some_and(|run| {
                !run.wide
                    && run.col + run.width == col
                    && (
                        run.fg,
                        run.bg,
                        run.faint,
                        run.bold,
                        run.italic,
                        run.underline,
                        run.strikethrough,
                    ) == (
                        attrs.fg,
                        attrs.bg,
                        attrs.faint,
                        attrs.bold,
                        attrs.italic,
                        attrs.underline,
                        attrs.strikethrough,
                    )
            });

        if mergeable {
            let run = runs.last_mut().expect("checked above");
            run.text.push(cell.char());
            run.width += 1;
        } else {
            runs.push(StyledRun {
                col,
                width: cell_width,
                text: cell.char().to_string(),
                fg: attrs.fg,
                bg: attrs.bg,
                faint: attrs.faint,
                bold: attrs.bold,
                italic: attrs.italic,
                underline: attrs.underline,
                strikethrough: attrs.strikethrough,
                wide,
            });
        }
        col += cell_width;
    }

    // Trailing blank cells share the text's default pen and merge into the
    // last run — trim them when nothing (bg/decoration) makes them visible.
    if let Some(run) = runs.last_mut()
        && run.bg.is_none()
        && !run.underline
        && !run.strikethrough
    {
        let trimmed = run.text.trim_end_matches(' ');
        run.width -= run.text.len() - trimmed.len();
        run.text.truncate(trimmed.len());
    }

    // Drop invisible runs: all-space runs with default bg and no decorations
    // contribute nothing (runs carry absolute columns).
    runs.retain(|run| !run.is_blank());
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(input: &str) -> Screen {
        interpret(input.as_bytes(), 80, 24)
    }

    #[test]
    fn plain_text_is_one_run() {
        let s = screen("hello");
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0].len(), 1);
        assert_eq!(s.rows[0][0].text, "hello");
        assert_eq!(s.rows[0][0].col, 0);
    }

    #[test]
    fn sgr_color_splits_runs() {
        let s = screen("a\x1b[31mred\x1b[0mb");
        let row = &s.rows[0];
        assert_eq!(row.len(), 3);
        assert_eq!(row[1].text, "red");
        assert_eq!(row[1].fg, PenColor::Indexed(1));
        assert_eq!(row[2].col, 4);
    }

    #[test]
    fn color_resolution_stays_symbolic_where_it_matters() {
        // Palette colors stay symbolic; the 256 cube, grayscale ramp, and
        // truecolor are theme-independent and collapse to RGB up front.
        let s = screen("\x1b[38;5;9ma\x1b[38;5;196mb\x1b[38;5;232mc\x1b[38;2;1;2;3md");
        let row = &s.rows[0];
        assert_eq!(row[0].fg, PenColor::Indexed(9));
        assert_eq!(row[1].fg, PenColor::Rgb(Rgb::new(255, 0, 0)));
        assert_eq!(row[2].fg, PenColor::Rgb(Rgb::new(8, 8, 8)));
        assert_eq!(row[3].fg, PenColor::Rgb(Rgb::new(1, 2, 3)));
    }

    #[test]
    fn faint_is_a_flag_not_a_blend() {
        let s = screen("\x1b[2mdim\x1b[0m bright");
        let row = &s.rows[0];
        assert!(row[0].faint);
        assert_eq!(row[0].fg, PenColor::DefaultFg);
        assert!(!row[1].faint);
    }

    #[test]
    fn carriage_return_overwrites() {
        let s = screen("00%\r50%\r99%");
        assert_eq!(s.rows[0][0].text, "99%");
        assert_eq!(s.rows.len(), 1);
    }

    #[test]
    fn inverse_swaps_colors() {
        let s = screen("\x1b[7mX\x1b[0m");
        let run = &s.rows[0][0];
        assert_eq!(run.fg, PenColor::DefaultBg);
        assert_eq!(run.bg, Some(PenColor::DefaultFg));
    }

    #[test]
    fn wide_chars_get_own_runs() {
        let s = screen("a漢b");
        let row = &s.rows[0];
        assert_eq!(row.len(), 3);
        assert_eq!(row[1].text, "漢");
        assert!(row[1].wide);
        assert_eq!(row[1].width, 2);
        assert_eq!(row[2].col, 3);
    }

    #[test]
    fn trailing_blank_lines_trimmed() {
        let s = screen("one\n\n\n");
        assert_eq!(s.rows.len(), 1);
    }

    #[test]
    fn cursor_up_redraw_resolves() {
        // draw two lines, move up, overwrite the first
        let s = screen("aaa\nbbb\x1b[1A\rccc\x1b[1B\n");
        assert_eq!(s.rows[0][0].text, "ccc");
        assert_eq!(s.rows[1][0].text, "bbb");
    }

    #[test]
    fn osc_title_detection() {
        // OSC 2 with BEL terminator.
        assert_eq!(osc_titles("\x1b]2;hello\x07rest"), ["hello"]);
        // OSC 0 with ST terminator.
        assert_eq!(osc_titles("\x1b]0;world\x1b\\"), ["world"]);
        // Titles arrive in order; other OSC codes are ignored.
        assert_eq!(
            osc_titles("\x1b]2;first\x07\x1b]7;file://x\x1b\\\x1b]2;second\x07"),
            ["first", "second"]
        );
        // Empty titles and unterminated sequences don't count.
        assert!(osc_titles("\x1b]2;\x07").is_empty());
        assert!(osc_titles("\x1b]2;dangling").is_empty());
        assert!(osc_titles("plain text").is_empty());
    }
}
