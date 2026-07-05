use crate::theme::Rgb;

/// A horizontal run of cells sharing identical resolved attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledRun {
    /// Starting column (0-based).
    pub col: usize,
    /// Number of terminal columns covered (wide chars count 2).
    pub width: usize,
    pub text: String,
    pub fg: Rgb,
    /// None means the theme's default background (the window body).
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    /// True when the run is a single double-width character.
    pub wide: bool,
}

impl StyledRun {
    pub fn is_blank(&self) -> bool {
        self.bg.is_none()
            && !self.underline
            && !self.strikethrough
            && self.text.chars().all(|c| c == ' ')
    }
}

/// Render-ready screen: attribute-resolved runs, one Vec per row.
#[derive(Debug)]
pub struct Screen {
    pub cols: usize,
    pub rows: Vec<Vec<StyledRun>>,
}

impl Screen {
    /// Split runs so every char the embedded font does not cover (emoji)
    /// gets its own explicitly-positioned run: fallback fonts advance at
    /// their own widths, and per-run x coordinates stop that drift from
    /// shifting later columns.
    pub fn split_uncovered(&mut self, covered: impl Fn(char) -> bool) {
        for row in &mut self.rows {
            if row
                .iter()
                .all(|run| run.text.chars().all(|c| c == ' ' || covered(c)))
            {
                continue;
            }
            let mut new_runs = Vec::with_capacity(row.len());
            for run in row.drain(..) {
                split_run(run, &covered, &mut new_runs);
            }
            *row = new_runs;
        }
    }
}

fn split_run(run: StyledRun, covered: &impl Fn(char) -> bool, out: &mut Vec<StyledRun>) {
    if run.text.chars().all(|c| c == ' ' || covered(c)) {
        out.push(run);
        return;
    }
    // Wide runs are always single-char, so chars here advance 1 column each.
    let char_w = if run.wide { 2 } else { 1 };
    let mut col = run.col;
    let mut pending = String::new();
    let mut pending_col = run.col;

    let flush = |text: &mut String, col: usize, out: &mut Vec<StyledRun>| {
        if text.is_empty() {
            return;
        }
        out.push(StyledRun {
            col,
            width: text.chars().count() * char_w,
            text: std::mem::take(text),
            ..run.clone()
        });
    };

    for c in run.text.chars() {
        if covered(c) || c == ' ' {
            if pending.is_empty() {
                pending_col = col;
            }
            pending.push(c);
        } else {
            flush(&mut pending, pending_col, out);
            out.push(StyledRun {
                col,
                width: char_w,
                text: c.to_string(),
                ..run.clone()
            });
        }
        col += char_w;
    }
    flush(&mut pending, pending_col, out);
}
