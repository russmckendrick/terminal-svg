pub mod anim;
pub mod chrome;
pub mod metrics;
pub mod svg;
pub mod text;

pub use anim::render_animated;
pub use chrome::ChromeStyle;
pub use svg::{render, render_dual};

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
}

#[derive(Debug, Clone)]
pub struct FontFaces {
    pub family: String,
    /// (CSS font-weight, base64 WOFF2) pairs.
    pub faces: Vec<(u16, String)>,
}

pub const DEFAULT_FONT_STACK: &str = "Menlo,Consolas,'DejaVu Sans Mono',monospace,'Apple Color Emoji','Segoe UI Emoji','Noto Color Emoji'";
