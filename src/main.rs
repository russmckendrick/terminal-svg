use anyhow::{Result, bail};
use clap::Parser;

use terminal_svg::cli::{AnimArgs, Cli, StyleArgs, Sub};
use terminal_svg::term::screen::Screen;
use terminal_svg::{anim, capture, cast, font, render, term, theme};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(Sub::Rec(rec)) = &cli.sub {
        return run_rec(rec);
    }

    if cli.list_themes {
        for name in theme::builtin::names() {
            println!("{name}");
        }
        return Ok(());
    }

    if let Some(path) = cli.input.as_deref().filter(|p| cast::looks_like_cast(p)) {
        let (header, events) = cast::read(path)?;
        return render_cast(header, &events, &cli.style, &cli.anim);
    }

    let source = if !cli.command.is_empty() {
        capture::Source::Command(cli.command.clone())
    } else if let Some(path) = &cli.input {
        capture::Source::File(path.clone())
    } else {
        capture::Source::Stdin
    };
    let captured = capture::capture(source, cli.cols as u16, cli.rows as u16, cli.timeout)?;

    // Title precedence: --title, then a title the program set via OSC 0/2,
    // then the captured command string.
    let title = resolve_title(
        &cli.style,
        [
            best_osc_title(&String::from_utf8_lossy(&captured.bytes)),
            captured.title.clone(),
        ],
    );
    render_static_bytes(&captured.bytes, cli.cols, cli.rows, title, &cli.style, None)
}

/// Load a theme by name or path; the reserved name "auto" resolves to the
/// palette embedded in the recording (asciicast v3 `term.theme`).
fn load_theme(name: &str, cast_theme: Option<&cast::CastTheme>) -> Result<theme::Theme> {
    if name == "auto" {
        let Some(t) = cast_theme else {
            bail!(
                "--theme auto needs an asciicast recording with an embedded theme \
                 (asciinema 3 records one; older casts and other inputs carry none)"
            );
        };
        return theme::Theme::from_palette("auto", t.fg, t.bg, &t.palette);
    }
    theme::builtin::load(name)
}

/// `terminal-svg rec`: record an interactive session to a .cast, then
/// render it as an animated SVG.
fn run_rec(rec: &terminal_svg::cli::RecArgs) -> Result<()> {
    if rec.style.output == "-" {
        bail!("rec cannot write the SVG to stdout (the session runs there); pass -o <file>");
    }
    let cast_path = rec.cast_path();
    let opts = capture::record::RecordOptions {
        command: rec.command.clone(),
        cols: rec.cols,
        rows: rec.rows,
    };
    capture::record::record(&cast_path, &opts)?;

    // Re-read the cast rather than trusting in-memory state: the file is
    // the recording of record, and this proves it round-trips.
    let (header, events) = cast::read(&cast_path)?;
    render_cast(header, &events, &rec.style, &rec.anim)
}

/// Resolve the window title: --title wins untouched (bar an explicit
/// --title-emoji); otherwise the first available fallback, decorated
/// Ghostty-style with a folder emoji when it looks like a path. The folder
/// treatment only fits the macOS chrome — Windows and Ubuntu bars show the
/// title as-is, like the real terminals do.
fn resolve_title(
    style: &StyleArgs,
    fallbacks: impl IntoIterator<Item = Option<String>>,
) -> Option<String> {
    let (title, from_user) = match &style.title {
        Some(t) => (Some(t.clone()), true),
        None => (fallbacks.into_iter().flatten().next(), false),
    };
    let auto_folder = !from_user && style.chrome() == render::ChromeStyle::Macos;
    decorate_title(title, style.title_emoji.as_deref(), auto_folder)
}

fn decorate_title(title: Option<String>, emoji: Option<&str>, auto_folder: bool) -> Option<String> {
    match emoji {
        // Explicitly disabled.
        Some("") => title,
        // Explicit emoji applies to any title, even a bare one.
        Some(e) => Some(match title {
            Some(t) => format!("{e} {t}"),
            None => e.to_string(),
        }),
        None => {
            let t = title?;
            if !auto_folder {
                return Some(t);
            }
            // Auto-detected titles that name a directory render like
            // Ghostty: "📁 ~/Code/blog", stripping any user@host: prefix.
            if let Some(p) = path_component(&t) {
                let decorated = format!("📁 {p}");
                return Some(decorated);
            }
            Some(t)
        }
    }
}

/// The directory a title points at, when it looks like one: "~/x", "/x",
/// or the path in "user@host:~/x".
fn path_component(title: &str) -> Option<&str> {
    if title.starts_with('~') || title.starts_with('/') {
        return Some(title);
    }
    title
        .rsplit_once(':')
        .map(|(_, p)| p.trim())
        .filter(|p| p.starts_with('~') || p.starts_with('/'))
}

/// The best OSC 0/2 title in a stream. Shells flip the title between the
/// running command and a "user@host:pwd" prompt title, so the most recent
/// path-style title wins — a recording that ends with `exit` shouldn't be
/// titled "exit".
fn best_osc_title(data: &str) -> Option<String> {
    let titles = term::osc_titles(data);
    titles
        .iter()
        .rev()
        .find(|t| path_component(t).is_some())
        .or(titles.last())
        .cloned()
}

/// Interpret raw ANSI bytes and render a static SVG — one theme, or a
/// dual light/dark document when --theme-light/--theme-dark are set.
fn render_static_bytes(
    bytes: &[u8],
    cols: usize,
    rows: usize,
    title: Option<String>,
    style: &StyleArgs,
    cast_theme: Option<&cast::CastTheme>,
) -> Result<()> {
    if let Some((light_name, dark_name)) = style.dual_themes() {
        let light = load_theme(light_name, cast_theme)?;
        let dark = load_theme(dark_name, cast_theme)?;
        let mut screen_l = term::interpret(bytes, cols, rows, &light);
        let mut screen_d = term::interpret(bytes, cols, rows, &dark);

        let (font_family, font_faces) = if style.no_font_embed {
            (referenced_family(style), None)
        } else {
            let covered = font::subset::coverage(font::assets::regular())?;
            screen_l.split_uncovered(&covered);
            screen_d.split_uncovered(&covered);
            let faces = embedded_faces([&screen_l, &screen_d].into_iter())?;
            (font::EMBEDDED_FONT_STACK.to_string(), faces)
        };

        let config = render_config(style, title, font_family, font_faces);
        let svg = render::render_dual((&screen_l, &light), (&screen_d, &dark), &config)?;
        return write_output(&style.output, &svg);
    }

    let theme = load_theme(&style.theme, cast_theme)?;
    let screen = term::interpret(bytes, cols, rows, &theme);
    render_static(screen, title, &theme, style)
}

/// The v1 path: render one screen to a static SVG and write it out.
fn render_static(
    mut screen: Screen,
    title: Option<String>,
    theme: &theme::Theme,
    style: &StyleArgs,
) -> Result<()> {
    let (font_family, font_faces) = if style.no_font_embed {
        (referenced_family(style), None)
    } else {
        let covered = font::subset::coverage(font::assets::regular())?;
        screen.split_uncovered(&covered);
        let faces = embedded_faces(std::iter::once(&screen))?;
        (font::EMBEDDED_FONT_STACK.to_string(), faces)
    };

    let config = render_config(style, title, font_family, font_faces);
    let svg = render::render(&screen, theme, &config)?;
    write_output(&style.output, &svg)
}

/// Render an asciicast to an animated SVG (or a static screen with
/// --static / --at).
fn render_cast(
    header: cast::Header,
    events: &[cast::Event],
    style: &StyleArgs,
    anim_args: &AnimArgs,
) -> Result<()> {
    if anim_args.is_static() {
        // A single point in time (--at, or the end): concatenate the
        // output and reuse the v1 path (scrollback included, trailing
        // blank rows trimmed).
        let mut bytes = Vec::new();
        for event in events {
            if anim_args.at.is_some_and(|at| event.time > at) {
                break;
            }
            if let cast::EventData::Output(data) = &event.data {
                bytes.extend_from_slice(data.as_bytes());
            }
        }
        let title = resolve_title(
            style,
            [
                header.title.clone(),
                best_osc_title(&String::from_utf8_lossy(&bytes)),
            ],
        );
        return render_static_bytes(
            &bytes,
            header.width,
            header.height,
            title,
            style,
            header.theme.as_ref(),
        );
    }

    if style.dual_themes().is_some() {
        bail!(
            "--theme-light/--theme-dark need --static for cast inputs (animated dual-theme SVGs are not supported yet)"
        );
    }
    let theme = load_theme(&style.theme, header.theme.as_ref())?;

    let osc_title = {
        let mut all = String::new();
        for event in events {
            if let cast::EventData::Output(data) = &event.data {
                all.push_str(data);
            }
        }
        best_osc_title(&all)
    };
    let title = resolve_title(style, [header.title.clone(), osc_title]);

    let opts = anim::AnimOptions {
        idle_time_limit: anim_args.idle_time_limit,
        speed: anim_args.speed,
    };
    let mut animation = anim::build_frames(&header, events, &theme, &opts);

    let (font_family, font_faces) = if style.no_font_embed {
        (referenced_family(style), None)
    } else {
        let covered = font::subset::coverage(font::assets::regular())?;
        for frame in &mut animation.frames {
            frame.screen.split_uncovered(&covered);
        }
        let faces = embedded_faces(animation.frames.iter().map(|f| &f.screen))?;
        (font::EMBEDDED_FONT_STACK.to_string(), faces)
    };

    let config = render_config(style, title, font_family, font_faces);
    let svg = render::render_animated(&animation, &theme, &config, !anim_args.no_loop)?;
    write_output(&style.output, &svg)
}

fn referenced_family(style: &StyleArgs) -> String {
    style
        .font_family
        .clone()
        .unwrap_or_else(|| render::DEFAULT_FONT_STACK.to_string())
}

fn render_config(
    style: &StyleArgs,
    title: Option<String>,
    font_family: String,
    font_faces: Option<render::FontFaces>,
) -> render::RenderConfig {
    render::RenderConfig {
        font_size: style.font_size,
        line_height: style.line_height,
        padding: style.padding,
        margin: style.margin(),
        chrome: style.chrome(),
        background: !style.no_background,
        shadow: style.shadow(),
        title,
        font_family,
        font_faces,
    }
}

fn write_output(output: &str, svg: &str) -> Result<()> {
    if output == "-" {
        print!("{svg}");
    } else {
        std::fs::write(output, svg)?;
        eprintln!("wrote {output}");
    }
    Ok(())
}

/// Collect the chars each face must cover across every screen (bold runs
/// subset into the bold face, everything else into regular) and build the
/// base64 WOFF2 @font-face payloads. Chrome title text renders in system
/// sans fonts, so it never needs the embedded mono subset.
fn embedded_faces<'a>(
    screens: impl Iterator<Item = &'a Screen>,
) -> Result<Option<render::FontFaces>> {
    use std::collections::BTreeSet;

    let mut regular: BTreeSet<char> = BTreeSet::new();
    let mut bold: BTreeSet<char> = BTreeSet::new();
    for run in screens.flat_map(|s| s.rows.iter()).flatten() {
        let set = if run.bold { &mut bold } else { &mut regular };
        set.extend(run.text.chars());
    }

    let mut faces = Vec::new();
    if let Some(b64) = font::subset::woff2_base64(font::assets::regular(), &regular)? {
        faces.push((400, b64));
    }
    if let Some(b64) = font::subset::woff2_base64(font::assets::bold(), &bold)? {
        faces.push((700, b64));
    }
    if faces.is_empty() {
        return Ok(None);
    }
    Ok(Some(render::FontFaces {
        family: font::EMBEDDED_FAMILY.to_string(),
        faces,
    }))
}

#[cfg(test)]
mod tests {
    use super::{best_osc_title, decorate_title};

    fn auto(title: &str) -> Option<String> {
        decorate_title(Some(title.into()), None, true)
    }

    #[test]
    fn auto_titles_get_folder_emoji_for_paths() {
        assert_eq!(auto("~/Code/blog").as_deref(), Some("📁 ~/Code/blog"));
        assert_eq!(auto("/etc").as_deref(), Some("📁 /etc"));
        // user@host:path strips down to the path.
        assert_eq!(
            auto("russ@mbp:~/Code/blog").as_deref(),
            Some("📁 ~/Code/blog")
        );
        // Non-path titles pass through.
        assert_eq!(auto("vim").as_deref(), Some("vim"));
        assert_eq!(auto("make: build").as_deref(), Some("make: build"));
    }

    #[test]
    fn titles_untouched_without_auto_folder_unless_emoji_given() {
        // No auto-folder (user title, or windows/ubuntu chrome).
        assert_eq!(
            decorate_title(Some("~/Code".into()), None, false).as_deref(),
            Some("~/Code")
        );
        assert_eq!(
            decorate_title(Some("demo".into()), Some("🚀"), false).as_deref(),
            Some("🚀 demo")
        );
        // An emoji with no title stands alone; empty string disables.
        assert_eq!(
            decorate_title(None, Some("🚀"), true).as_deref(),
            Some("🚀")
        );
        assert_eq!(
            decorate_title(Some("~/x".into()), Some(""), true).as_deref(),
            Some("~/x")
        );
        assert_eq!(decorate_title(None, None, true), None);
    }

    #[test]
    fn prompt_path_title_beats_last_command() {
        // zsh sets the title to each running command; the prompt-style
        // path title should win over a trailing "exit".
        let data = "\x1b]2;russ@mbp:~/Code/blog\x07\x1b]2;git pull\x07\x1b]2;exit\x07";
        assert_eq!(
            best_osc_title(data).as_deref(),
            Some("russ@mbp:~/Code/blog")
        );
        // With no path-style title, the last one still wins.
        assert_eq!(
            best_osc_title("\x1b]2;vim\x07\x1b]2;htop\x07").as_deref(),
            Some("htop")
        );
    }
}
