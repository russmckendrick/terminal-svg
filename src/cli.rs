use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::render::ChromeStyle;

/// Render terminal output as a pixel-perfect SVG screenshot.
///
/// Reads ANSI from a file or stdin, or runs a command in a PTY:
///   cmd | terminal-svg -o shot.svg
///   terminal-svg dump.ansi
///   terminal-svg -- lsd -la
///
/// A .cast input (asciicast v2 or v3, e.g. from `terminal-svg rec` or
/// asciinema) renders as an animated SVG replaying the recording:
///   terminal-svg demo.cast -o demo.svg
#[derive(Debug, Parser)]
#[command(name = "terminal-svg", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub sub: Option<Sub>,

    /// Input file with ANSI output or an asciicast (v2/v3) recording;
    /// stdin is read when omitted and no command is given
    pub input: Option<PathBuf>,

    /// Command to run in a PTY and capture (everything after --)
    #[arg(last = true)]
    pub command: Vec<String>,

    #[command(flatten)]
    pub style: StyleArgs,

    /// Terminal width in columns
    #[arg(short, long, default_value_t = 80)]
    pub cols: usize,

    /// Terminal height in rows (PTY size; output height follows content)
    #[arg(short, long, default_value_t = 24)]
    pub rows: usize,

    /// Kill the PTY command after this many seconds
    #[arg(long)]
    pub timeout: Option<u64>,

    /// List built-in themes and exit
    #[arg(long)]
    pub list_themes: bool,

    #[command(flatten)]
    pub anim: AnimArgs,
}

#[derive(Debug, Subcommand)]
pub enum Sub {
    /// Record an interactive terminal session and render it as an
    /// animated SVG when the session ends
    Rec(RecArgs),
}

/// Flags shared by the one-shot renderer and `rec`; the parsed surface is
/// identical to the pre-subcommand CLI.
#[derive(Debug, Args)]
pub struct StyleArgs {
    /// Output path ("-" writes the SVG to stdout)
    #[arg(short, long, default_value = "terminal.svg")]
    pub output: String,

    /// Theme name, path to a custom theme .toml, or "auto" to use the
    /// palette embedded in an asciicast v3 recording
    #[arg(short, long, default_value = "dracula")]
    pub theme: String,

    /// Light theme for a dual light/dark SVG switched by
    /// prefers-color-scheme (requires --theme-dark; static renders only)
    #[arg(long, value_name = "THEME", requires = "theme_dark")]
    pub theme_light: Option<String>,

    /// Dark theme for a dual light/dark SVG switched by
    /// prefers-color-scheme (requires --theme-light; static renders only)
    #[arg(long, value_name = "THEME", requires = "theme_light")]
    pub theme_dark: Option<String>,

    /// Window title (defaults to the recording's title for casts, or the
    /// command string for PTY captures)
    #[arg(long)]
    pub title: Option<String>,

    /// Emoji shown before the title (default: 📁 when the title is a
    /// path; pass an empty string to disable)
    #[arg(long, value_name = "EMOJI")]
    pub title_emoji: Option<String>,

    /// Window chrome style; fixed-size like a real window, it does not
    /// scale with --font-size
    #[arg(long, value_enum, default_value = "macos")]
    pub chrome: ChromeStyle,

    /// Font size in px
    #[arg(long, default_value_t = 14.0)]
    pub font_size: f32,

    /// Line height as a multiple of font size
    #[arg(long, default_value_t = 1.2)]
    pub line_height: f32,

    /// Padding between window edge and text, in px
    #[arg(long, default_value_t = 10.0)]
    pub padding: f32,

    /// Margin around the window, in px (default 24, or 0 with --no-shadow)
    #[arg(long)]
    pub margin: Option<f32>,

    /// Render a bare panel without a title bar (alias for --chrome none)
    #[arg(long)]
    pub no_window: bool,

    /// Transparent background: no window body, chrome, or shadow
    #[arg(long)]
    pub no_background: bool,

    /// Disable the drop shadow
    #[arg(long)]
    pub no_shadow: bool,

    /// Reference system fonts instead of embedding a subsetted font
    #[arg(long)]
    pub no_font_embed: bool,

    /// Font family to reference with --no-font-embed
    #[arg(long)]
    pub font_family: Option<String>,
}

impl StyleArgs {
    pub fn margin(&self) -> f32 {
        self.margin
            .unwrap_or(if self.shadow() { 24.0 } else { 0.0 })
    }

    /// The effective chrome style: --no-window and --no-background both
    /// force a bare panel.
    pub fn chrome(&self) -> ChromeStyle {
        if self.no_window || self.no_background {
            ChromeStyle::None
        } else {
            self.chrome
        }
    }

    /// --no-background leaves nothing to cast a shadow.
    pub fn shadow(&self) -> bool {
        !self.no_shadow && !self.no_background
    }

    /// Both dual-theme names, when the pair is requested.
    pub fn dual_themes(&self) -> Option<(&str, &str)> {
        match (self.theme_light.as_deref(), self.theme_dark.as_deref()) {
            (Some(l), Some(d)) => Some((l, d)),
            _ => None,
        }
    }
}

/// Animation options; they apply when rendering an asciicast (a .cast input
/// or a `rec` session).
#[derive(Debug, Args)]
pub struct AnimArgs {
    /// Play the animation once and hold the last frame instead of looping
    #[arg(long)]
    pub no_loop: bool,

    /// Cap pauses between events at this many seconds
    /// (default: the recording's own limit, or 2)
    #[arg(long, value_name = "SECONDS")]
    pub idle_time_limit: Option<f64>,

    /// Playback speed multiplier (2 = twice as fast)
    #[arg(long, default_value_t = 1.0)]
    pub speed: f64,

    /// Render only the final frame as a static SVG
    #[arg(long = "static")]
    pub static_: bool,

    /// Render the screen as of this many seconds into the recording
    /// instead of the end (implies --static)
    #[arg(long, value_name = "SECONDS")]
    pub at: Option<f64>,
}

impl AnimArgs {
    pub fn is_static(&self) -> bool {
        self.static_ || self.at.is_some()
    }
}

#[derive(Debug, Args)]
pub struct RecArgs {
    /// Command to record (everything after --; defaults to $SHELL)
    #[arg(last = true)]
    pub command: Vec<String>,

    /// Where to save the asciicast recording
    /// (default: the output path with a .cast extension)
    #[arg(long)]
    pub cast: Option<PathBuf>,

    /// Terminal width in columns (defaults to the current terminal's)
    #[arg(short, long)]
    pub cols: Option<u16>,

    /// Terminal height in rows (defaults to the current terminal's)
    #[arg(short, long)]
    pub rows: Option<u16>,

    #[command(flatten)]
    pub style: StyleArgs,

    #[command(flatten)]
    pub anim: AnimArgs,
}

impl RecArgs {
    /// The .cast path: --cast, or the SVG output with a .cast extension
    /// ("terminal.cast" when writing the SVG to stdout).
    pub fn cast_path(&self) -> PathBuf {
        if let Some(path) = &self.cast {
            return path.clone();
        }
        if self.style.output == "-" {
            return PathBuf::from("terminal.cast");
        }
        PathBuf::from(&self.style.output).with_extension("cast")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    // The v1 (pre-subcommand) invocations must keep parsing identically.

    #[test]
    fn v1_stdin_defaults() {
        let cli = parse(&["terminal-svg"]);
        assert!(cli.sub.is_none());
        assert!(cli.input.is_none());
        assert!(cli.command.is_empty());
        assert_eq!(cli.style.output, "terminal.svg");
        assert_eq!(cli.style.theme, "dracula");
        assert_eq!((cli.cols, cli.rows), (80, 24));
        assert_eq!(cli.style.font_size, 14.0);
        assert_eq!(cli.style.margin(), 24.0);
    }

    #[test]
    fn v1_file_input_with_flags() {
        let cli = parse(&[
            "terminal-svg",
            "dump.ansi",
            "-o",
            "shot.svg",
            "-t",
            "nord",
            "-c",
            "100",
            "-r",
            "30",
            "--font-size",
            "16",
            "--no-shadow",
            "--title",
            "demo",
        ]);
        assert!(cli.sub.is_none());
        assert_eq!(cli.input.unwrap(), PathBuf::from("dump.ansi"));
        assert_eq!(cli.style.output, "shot.svg");
        assert_eq!(cli.style.theme, "nord");
        assert_eq!((cli.cols, cli.rows), (100, 30));
        assert_eq!(cli.style.font_size, 16.0);
        assert_eq!(cli.style.margin(), 0.0);
        assert_eq!(cli.style.title.as_deref(), Some("demo"));
    }

    #[test]
    fn v1_pty_command() {
        let cli = parse(&["terminal-svg", "--timeout", "5", "--", "lsd", "-la"]);
        assert!(cli.sub.is_none());
        assert_eq!(cli.command, vec!["lsd", "-la"]);
        assert_eq!(cli.timeout, Some(5));
    }

    #[test]
    fn v1_list_themes() {
        assert!(parse(&["terminal-svg", "--list-themes"]).list_themes);
    }

    #[test]
    fn cast_input_with_anim_flags() {
        let cli = parse(&["terminal-svg", "demo.cast", "--speed", "2", "--no-loop"]);
        assert_eq!(cli.input.unwrap(), PathBuf::from("demo.cast"));
        assert!(cli.anim.no_loop);
        assert_eq!(cli.anim.speed, 2.0);
        assert_eq!(cli.anim.idle_time_limit, None);
        assert!(!cli.anim.static_);
    }

    #[test]
    fn rec_defaults() {
        let cli = parse(&["terminal-svg", "rec"]);
        let Some(Sub::Rec(rec)) = cli.sub else {
            panic!("expected rec subcommand");
        };
        assert!(rec.command.is_empty());
        assert_eq!(rec.cols, None);
        assert_eq!(rec.rows, None);
        assert_eq!(rec.cast_path(), PathBuf::from("terminal.cast"));
    }

    #[test]
    fn rec_with_command_and_flags() {
        let cli = parse(&[
            "terminal-svg",
            "rec",
            "-o",
            "demo.svg",
            "--idle-time-limit",
            "3",
            "--",
            "zsh",
            "-i",
        ]);
        let Some(Sub::Rec(rec)) = cli.sub else {
            panic!("expected rec subcommand");
        };
        assert_eq!(rec.command, vec!["zsh", "-i"]);
        assert_eq!(rec.style.output, "demo.svg");
        assert_eq!(rec.anim.idle_time_limit, Some(3.0));
        assert_eq!(rec.cast_path(), PathBuf::from("demo.cast"));
    }

    #[test]
    fn rec_cast_path_override() {
        let cli = parse(&["terminal-svg", "rec", "--cast", "take2.cast"]);
        let Some(Sub::Rec(rec)) = cli.sub else {
            panic!("expected rec subcommand");
        };
        assert_eq!(rec.cast_path(), PathBuf::from("take2.cast"));
    }

    #[test]
    fn chrome_flag_and_aliases() {
        assert_eq!(parse(&["terminal-svg"]).style.chrome(), ChromeStyle::Macos);
        assert_eq!(
            parse(&["terminal-svg", "--chrome", "windows"])
                .style
                .chrome(),
            ChromeStyle::Windows
        );
        assert_eq!(
            parse(&["terminal-svg", "--no-window"]).style.chrome(),
            ChromeStyle::None
        );

        // --no-background forces a bare panel with no shadow or margin.
        let style = parse(&["terminal-svg", "--chrome", "ubuntu", "--no-background"]).style;
        assert_eq!(style.chrome(), ChromeStyle::None);
        assert!(!style.shadow());
        assert_eq!(style.margin(), 0.0);
    }

    #[test]
    fn dual_theme_flags_require_each_other() {
        assert!(Cli::try_parse_from(["terminal-svg", "--theme-light", "github-light"]).is_err());
        let style = parse(&[
            "terminal-svg",
            "--theme-light",
            "github-light",
            "--theme-dark",
            "github-dark",
        ])
        .style;
        assert_eq!(style.dual_themes(), Some(("github-light", "github-dark")));
    }

    #[test]
    fn at_implies_static() {
        let cli = parse(&["terminal-svg", "demo.cast", "--at", "2.5"]);
        assert!(cli.anim.is_static());
        assert_eq!(cli.anim.at, Some(2.5));
        assert!(!parse(&["terminal-svg", "demo.cast"]).anim.is_static());
    }

    #[test]
    fn rec_like_filename_is_not_the_subcommand() {
        let cli = parse(&["terminal-svg", "rec.cast"]);
        assert!(cli.sub.is_none());
        assert_eq!(cli.input.unwrap(), PathBuf::from("rec.cast"));
    }
}
