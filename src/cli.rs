use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::options::RenderOptions;
use crate::render::{ChromeStyle, CursorStyle};

const EXAMPLES: &str = "\
Examples:
  ls --color=always | terminal-svg -o ls.svg    pipe ANSI output through
  terminal-svg -- lsd -la                       run a command in a PTY
  terminal-svg dump.ansi -t nord                render a captured ANSI file
  terminal-svg rec -o demo.svg                  record your shell, render on exit
  terminal-svg demo.cast --speed 2              replay a recording as animated SVG
  terminal-svg demo.cast --theme-light github-light --theme-dark github-dark
  terminal-svg demo.svg -t nord                 re-render an SVG (it carries its source)
  terminal-svg extract demo.svg                 recover the recording from an SVG
  terminal-svg edit demo.cast --redact 'ghp_\\w+' -o clean.cast
  terminal-svg editor demo.cast                 tweak every option live in the browser

Docs & recipes: https://github.com/russmckendrick/terminal-svg/tree/main/docs";

/// Render terminal output as a pixel-perfect SVG screenshot.
///
/// Reads ANSI from a file or stdin, or runs a command in a PTY so
/// programs see a real terminal and switch on color. An asciicast
/// recording (v2 or v3, from `terminal-svg rec` or asciinema) renders as
/// an animated SVG that plays anywhere an <img> tag does.
#[derive(Debug, Parser)]
#[command(
    name = "terminal-svg",
    version,
    about,
    subcommand_value_name = "SUBCOMMAND",
    after_help = EXAMPLES
)]
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
    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 80,
        help_heading = "Capture"
    )]
    pub cols: usize,

    /// Terminal height in rows (PTY size; output height follows content)
    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 24,
        help_heading = "Capture"
    )]
    pub rows: usize,

    /// Kill the PTY command after this many seconds
    #[arg(long, value_name = "SECONDS", help_heading = "Capture")]
    pub timeout: Option<u64>,

    /// List built-in themes and exit
    #[arg(long)]
    pub list_themes: bool,

    /// Generate shell completions and exit
    #[arg(long, value_name = "SHELL", value_enum)]
    pub completions: Option<clap_complete::Shell>,

    /// Write the man page (roff) to stdout and exit
    #[arg(long, hide = true)]
    pub man: bool,

    /// Config file with default flag values
    /// (default: ~/.config/terminal-svg/config.toml, when it exists)
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(flatten)]
    pub anim: AnimArgs,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)] // one Sub exists per process
pub enum Sub {
    /// Record an interactive terminal session and render it as an
    /// animated SVG when the session ends
    Rec(RecArgs),
    /// Recover the recording embedded in a terminal-svg SVG
    /// (the source is embedded by default; see --no-embed-source)
    Extract(ExtractArgs),
    /// Clean up a recording without re-recording: mask secrets, cut time
    /// ranges, clamp long pauses
    Edit(EditArgs),
    /// Open a recording in the visual editor: a local web page with live
    /// preview of every render option
    Editor(EditorArgs),
}

#[derive(Debug, Args)]
pub struct EditorArgs {
    /// Recording (.cast), ANSI dump, or terminal-svg SVG to open
    /// (a file can also be dropped onto the page later)
    pub input: Option<PathBuf>,

    /// Port for the local editor server (default: any free port)
    #[arg(
        long,
        value_name = "PORT",
        default_value_t = 0,
        help_heading = "Editor"
    )]
    pub port: u16,

    /// Print the URL without opening the browser
    #[arg(long, help_heading = "Editor")]
    pub no_open: bool,

    #[command(flatten)]
    pub style: StyleArgs,

    #[command(flatten)]
    pub anim: AnimArgs,
}

#[derive(Debug, Args)]
pub struct ExtractArgs {
    /// SVG file written by terminal-svg
    pub input: PathBuf,

    /// Where to write the recovered .cast or .ansi source
    /// (default: stdout)
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct EditArgs {
    /// Recording to edit (.cast, v2 or v3; the output keeps the version)
    pub input: PathBuf,

    /// Where to write the edited recording ("-" for stdout; editing in
    /// place is refused so a bad pattern can't destroy the original)
    #[arg(short, long, value_name = "PATH")]
    pub output: String,

    /// Mask matches of this regex with '*' (repeatable); matched across
    /// event boundaries in both the output and input streams
    #[arg(long, value_name = "REGEX")]
    pub redact: Vec<String>,

    /// Remove a time range, e.g. --cut 12.5-20 (repeatable, seconds on
    /// the original recording's timeline)
    #[arg(long, value_name = "FROM-TO", value_parser = parse_cut_range)]
    pub cut: Vec<(f64, f64)>,

    /// Clamp pauses between events to this many seconds, baked into the
    /// recording (unlike --idle-time-limit, which only affects a render)
    #[arg(long, value_name = "SECONDS")]
    pub max_pause: Option<f64>,
}

fn parse_cut_range(s: &str) -> Result<(f64, f64), String> {
    let (from, to) = s
        .split_once('-')
        .ok_or_else(|| format!("expected FROM-TO, got {s:?}"))?;
    let from: f64 = from
        .trim()
        .parse()
        .map_err(|_| format!("invalid FROM in {s:?}"))?;
    let to: f64 = to
        .trim()
        .parse()
        .map_err(|_| format!("invalid TO in {s:?}"))?;
    if from < 0.0 {
        return Err(format!("FROM must be non-negative in {s:?}"));
    }
    if to <= from {
        return Err(format!("TO must be greater than FROM in {s:?}"));
    }
    Ok((from, to))
}

/// Flags shared by the one-shot renderer and `rec`; the parsed surface is
/// identical to the pre-subcommand CLI.
#[derive(Debug, Args)]
pub struct StyleArgs {
    /// Output path ("-" writes the SVG to stdout)
    #[arg(
        short,
        long,
        value_name = "PATH",
        default_value = "terminal.svg",
        help_heading = "Output & themes"
    )]
    pub output: String,

    /// Theme name, path to a custom theme .toml, or "auto" to use the
    /// palette embedded in an asciicast v3 recording
    #[arg(
        short,
        long,
        value_name = "THEME",
        default_value = "dracula",
        help_heading = "Output & themes"
    )]
    pub theme: String,

    /// Light theme for a dual light/dark SVG switched by
    /// prefers-color-scheme (requires --theme-dark)
    #[arg(
        long,
        value_name = "THEME",
        requires = "theme_dark",
        help_heading = "Output & themes"
    )]
    pub theme_light: Option<String>,

    /// Dark theme for a dual light/dark SVG switched by
    /// prefers-color-scheme (requires --theme-light)
    #[arg(
        long,
        value_name = "THEME",
        requires = "theme_light",
        help_heading = "Output & themes"
    )]
    pub theme_dark: Option<String>,

    /// Window title (defaults to the recording's title for casts, or the
    /// command string for PTY captures)
    #[arg(long, value_name = "TITLE", help_heading = "Window")]
    pub title: Option<String>,

    /// Emoji shown before the title (default: 📁 when the title is a
    /// path; pass an empty string to disable)
    #[arg(long, value_name = "EMOJI", help_heading = "Window")]
    pub title_emoji: Option<String>,

    /// Window chrome style; fixed-size like a real window, it does not
    /// scale with --font-size
    #[arg(
        long,
        value_name = "STYLE",
        value_enum,
        default_value = "macos",
        help_heading = "Window"
    )]
    pub chrome: ChromeStyle,

    /// Render a bare panel without a title bar (alias for --chrome none)
    #[arg(long, help_heading = "Window")]
    pub no_window: bool,

    /// Transparent background: no window body, chrome, or shadow
    #[arg(long, help_heading = "Window")]
    pub no_background: bool,

    /// Disable the drop shadow
    #[arg(long, help_heading = "Window")]
    pub no_shadow: bool,

    /// Font size in px
    #[arg(
        long,
        value_name = "PX",
        default_value_t = 14.0,
        help_heading = "Layout & fonts"
    )]
    pub font_size: f32,

    /// Line height as a multiple of font size
    #[arg(
        long,
        value_name = "N",
        default_value_t = 1.2,
        help_heading = "Layout & fonts"
    )]
    pub line_height: f32,

    /// Padding between window edge and text, in px
    #[arg(
        long,
        value_name = "PX",
        default_value_t = 10.0,
        help_heading = "Layout & fonts"
    )]
    pub padding: f32,

    /// Margin around the window, in px (default 24, or 0 with --no-shadow)
    #[arg(long, value_name = "PX", help_heading = "Layout & fonts")]
    pub margin: Option<f32>,

    /// Do not embed the source recording in the SVG metadata (embedding
    /// makes the SVG re-renderable and the recording recoverable with
    /// `extract`, but means the SVG carries everything that was captured)
    #[arg(long, help_heading = "Output & themes")]
    pub no_embed_source: bool,

    /// Reference system fonts instead of embedding a subsetted font
    #[arg(long, help_heading = "Layout & fonts")]
    pub no_font_embed: bool,

    /// Font family to reference with --no-font-embed
    #[arg(long, value_name = "NAME", help_heading = "Layout & fonts")]
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

    /// The render request these flags describe (post config merge).
    /// Values copy over verbatim rather than resolved — margin stays None
    /// so its shadow-dependent default keeps applying at render time.
    pub fn to_options(&self, anim: &AnimArgs) -> RenderOptions {
        RenderOptions {
            theme: Some(self.theme.clone()),
            theme_light: self.theme_light.clone(),
            theme_dark: self.theme_dark.clone(),
            chrome: Some(self.chrome),
            no_window: self.no_window,
            no_background: self.no_background,
            no_shadow: self.no_shadow,
            title: self.title.clone(),
            title_emoji: self.title_emoji.clone(),
            title_fallback: None,
            font_size: Some(self.font_size),
            line_height: Some(self.line_height),
            padding: Some(self.padding),
            margin: self.margin,
            no_font_embed: self.no_font_embed,
            font_family: self.font_family.clone(),
            cols: None,
            rows: None,
            cursor: Some(anim.cursor),
            speed: Some(anim.speed),
            idle_time_limit: anim.idle_time_limit,
            no_loop: anim.no_loop,
            from: anim.from,
            to: anim.to,
            static_: anim.static_,
            at: anim.at,
        }
    }
}

/// Animation options; they apply when rendering an asciicast (a .cast input
/// or a `rec` session).
#[derive(Debug, Args)]
#[command(next_help_heading = "Animation (cast inputs and rec)")]
pub struct AnimArgs {
    /// Play the animation once and hold the last frame instead of looping
    #[arg(long)]
    pub no_loop: bool,

    /// Cap pauses between events at this many seconds
    /// (default: the recording's own limit, or 2)
    #[arg(long, value_name = "SECONDS")]
    pub idle_time_limit: Option<f64>,

    /// Playback speed multiplier (2 = twice as fast)
    #[arg(long, value_name = "N", default_value_t = 1.0)]
    pub speed: f64,

    /// Start the animation this many seconds into the recording; the
    /// first frame shows the screen as of that moment
    #[arg(long, value_name = "SECONDS", conflicts_with_all = ["static_", "at"])]
    pub from: Option<f64>,

    /// End the animation at this many seconds into the recording
    #[arg(long, value_name = "SECONDS", conflicts_with_all = ["static_", "at"])]
    pub to: Option<f64>,

    /// Cursor shape in animated output
    #[arg(long, value_name = "STYLE", value_enum, default_value = "block")]
    pub cursor: CursorStyle,

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
    #[arg(long, value_name = "PATH", help_heading = "Capture")]
    pub cast: Option<PathBuf>,

    /// Terminal width in columns (defaults to the current terminal's)
    #[arg(short, long, value_name = "N", help_heading = "Capture")]
    pub cols: Option<u16>,

    /// Terminal height in rows (defaults to the current terminal's)
    #[arg(short, long, value_name = "N", help_heading = "Capture")]
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

    #[test]
    fn extract_like_filename_is_not_the_subcommand() {
        let cli = parse(&["terminal-svg", "extract.svg"]);
        assert!(cli.sub.is_none());
        assert_eq!(cli.input.unwrap(), PathBuf::from("extract.svg"));
    }

    #[test]
    fn extract_subcommand_parses() {
        let cli = parse(&["terminal-svg", "extract", "demo.svg", "-o", "demo.cast"]);
        let Some(Sub::Extract(args)) = cli.sub else {
            panic!("expected extract subcommand");
        };
        assert_eq!(args.input, PathBuf::from("demo.svg"));
        assert_eq!(args.output.unwrap(), PathBuf::from("demo.cast"));
    }

    #[test]
    fn no_embed_source_flag_parses() {
        assert!(
            parse(&["terminal-svg", "--no-embed-source"])
                .style
                .no_embed_source
        );
        assert!(!parse(&["terminal-svg"]).style.no_embed_source);
    }

    #[test]
    fn edit_subcommand_parses() {
        let cli = parse(&[
            "terminal-svg",
            "edit",
            "demo.cast",
            "--redact",
            r"ghp_\w+",
            "--cut",
            "12.5-20",
            "--max-pause",
            "1.5",
            "-o",
            "clean.cast",
        ]);
        let Some(Sub::Edit(args)) = cli.sub else {
            panic!("expected edit subcommand");
        };
        assert_eq!(args.input, PathBuf::from("demo.cast"));
        assert_eq!(args.output, "clean.cast");
        assert_eq!(args.redact, vec![r"ghp_\w+"]);
        assert_eq!(args.cut, vec![(12.5, 20.0)]);
        assert_eq!(args.max_pause, Some(1.5));

        // Malformed ranges are rejected at parse time.
        assert!(
            Cli::try_parse_from([
                "terminal-svg",
                "edit",
                "a.cast",
                "--cut",
                "20-12",
                "-o",
                "b"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from(["terminal-svg", "edit", "a.cast", "--cut", "oops", "-o", "b"])
                .is_err()
        );
    }

    #[test]
    fn editor_subcommand_parses() {
        let cli = parse(&[
            "terminal-svg",
            "editor",
            "demo.cast",
            "--port",
            "7391",
            "--no-open",
            "-t",
            "nord",
        ]);
        let Some(Sub::Editor(args)) = cli.sub else {
            panic!("expected editor subcommand");
        };
        assert_eq!(args.input.unwrap(), PathBuf::from("demo.cast"));
        assert_eq!(args.port, 7391);
        assert!(args.no_open);
        assert_eq!(args.style.theme, "nord");

        // Bare launch works; the page's drop zone takes it from there.
        let cli = parse(&["terminal-svg", "editor"]);
        let Some(Sub::Editor(args)) = cli.sub else {
            panic!("expected editor subcommand");
        };
        assert!(args.input.is_none());
        assert_eq!(args.port, 0);
    }

    #[test]
    fn completions_and_man_generate() {
        use clap::CommandFactory;

        let mut buf = Vec::new();
        clap_complete::generate(
            clap_complete::Shell::Zsh,
            &mut Cli::command(),
            "terminal-svg",
            &mut buf,
        );
        assert!(String::from_utf8(buf).unwrap().contains("terminal-svg"));

        let mut buf = Vec::new();
        clap_mangen::Man::new(Cli::command())
            .render(&mut buf)
            .unwrap();
        let man = String::from_utf8(buf).unwrap();
        assert!(man.contains(".TH") && man.contains("terminal-svg"));
    }
}
