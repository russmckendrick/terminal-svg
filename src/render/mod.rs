pub mod anim;
pub mod chrome;
pub mod metrics;
pub mod svg;
pub mod text;

pub use anim::{render_animated, render_animated_dual};
pub use chrome::ChromeStyle;
pub use svg::{render, render_dual};

/// How run colors become SVG attributes.
pub(crate) enum FillMode<'a> {
    /// Resolve to inline hex against one theme (static and single-theme
    /// animated output).
    Hex(&'a crate::theme::Theme),
    /// Emit palette classes (`cf`/`cb`/`c0`–`c15`) so a CSS block can
    /// switch the palette; theme-independent colors stay inline.
    Class,
}

/// The CSS class for a palette-dependent color; None for colors that are
/// the same in every theme.
pub(crate) fn palette_class(color: crate::term::screen::PenColor) -> Option<String> {
    use crate::term::screen::PenColor;
    match color {
        PenColor::DefaultFg => Some("cf".to_string()),
        PenColor::DefaultBg => Some("cb".to_string()),
        PenColor::Indexed(i) => Some(format!("c{i}")),
        PenColor::Rgb(_) => None,
    }
}

/// Cursor shape in animated output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum CursorStyle {
    #[default]
    Block,
    Bar,
    Underline,
    None,
}

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub font_size: f32,
    /// Line height as a multiple of font size.
    pub line_height: f32,
    /// Space between the window edge and the text grid.
    pub padding: f32,
    /// Space around the window (room for the shadow).
    pub margin: f32,
    /// Window chrome style (title bar and buttons).
    pub chrome: ChromeStyle,
    /// Draw the window body; false leaves the background transparent.
    pub background: bool,
    pub shadow: bool,
    pub title: Option<String>,
    /// CSS font-family chain for the terminal text.
    pub font_family: String,
    /// Base64 WOFF2 @font-face blocks (regular, bold), when embedding.
    pub font_faces: Option<FontFaces>,
    /// Cursor shape in animated output.
    pub cursor: CursorStyle,
}

#[derive(Debug, Clone)]
pub struct FontFaces {
    pub family: String,
    /// (CSS font-weight, base64 WOFF2) pairs.
    pub faces: Vec<(u16, String)>,
}

pub const DEFAULT_FONT_STACK: &str = "Menlo,Consolas,'DejaVu Sans Mono',monospace,'Apple Color Emoji','Segoe UI Emoji','Noto Color Emoji'";
