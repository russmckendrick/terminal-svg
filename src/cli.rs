use clap::Parser;
use std::path::PathBuf;

/// Render terminal output as a pixel-perfect SVG screenshot.
///
/// Reads ANSI from a file or stdin, or runs a command in a PTY:
///   cmd | terminal-svg -o shot.svg
///   terminal-svg dump.ansi
///   terminal-svg -- lsd -la
#[derive(Debug, Parser)]
#[command(name = "terminal-svg", version, about)]
pub struct Cli {
    /// Input file with ANSI output; stdin is read when omitted and no
    /// command is given
    pub input: Option<PathBuf>,

    /// Command to run in a PTY and capture (everything after --)
    #[arg(last = true)]
    pub command: Vec<String>,

    /// Output path ("-" writes the SVG to stdout)
    #[arg(short, long, default_value = "terminal.svg")]
    pub output: String,

    /// Theme name, or path to a custom theme .toml
    #[arg(short, long, default_value = "dracula")]
    pub theme: String,

    /// Window title (defaults to the command string for PTY captures)
    #[arg(long)]
    pub title: Option<String>,

    /// Terminal width in columns
    #[arg(short, long, default_value_t = 80)]
    pub cols: usize,

    /// Terminal height in rows (PTY size; output height follows content)
    #[arg(short, long, default_value_t = 24)]
    pub rows: usize,

    /// Font size in px
    #[arg(long, default_value_t = 14.0)]
    pub font_size: f32,

    /// Line height as a multiple of font size
    #[arg(long, default_value_t = 1.2)]
    pub line_height: f32,

    /// Padding between window edge and text, in px
    #[arg(long, default_value_t = 16.0)]
    pub padding: f32,

    /// Margin around the window, in px (default 24, or 0 with --no-shadow)
    #[arg(long)]
    pub margin: Option<f32>,

    /// Render a bare panel without title bar and traffic lights
    #[arg(long)]
    pub no_window: bool,

    /// Disable the drop shadow
    #[arg(long)]
    pub no_shadow: bool,

    /// Reference system fonts instead of embedding a subsetted font
    #[arg(long)]
    pub no_font_embed: bool,

    /// Font family to reference with --no-font-embed
    #[arg(long)]
    pub font_family: Option<String>,

    /// Kill the PTY command after this many seconds
    #[arg(long)]
    pub timeout: Option<u64>,

    /// List built-in themes and exit
    #[arg(long)]
    pub list_themes: bool,
}

impl Cli {
    pub fn margin(&self) -> f32 {
        self.margin
            .unwrap_or(if self.no_shadow { 0.0 } else { 24.0 })
    }
}
