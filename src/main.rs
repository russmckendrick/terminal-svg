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

    let theme = theme::builtin::load(&cli.style.theme)?;

    if let Some(path) = cli.input.as_deref().filter(|p| cast::looks_like_cast(p)) {
        let (header, events) = cast::read(path)?;
        return render_cast(header, &events, &theme, &cli.style, &cli.anim);
    }

    let source = if !cli.command.is_empty() {
        capture::Source::Command(cli.command.clone())
    } else if let Some(path) = &cli.input {
        capture::Source::File(path.clone())
    } else {
        capture::Source::Stdin
    };
    let captured = capture::capture(source, cli.cols as u16, cli.rows as u16, cli.timeout)?;

    let screen = term::interpret(&captured.bytes, cli.cols, cli.rows, &theme);
    let title = cli.style.title.clone().or(captured.title);
    render_static(screen, title, &theme, &cli.style)
}

/// `terminal-svg rec`: record an interactive session to a .cast, then
/// render it as an animated SVG.
fn run_rec(rec: &terminal_svg::cli::RecArgs) -> Result<()> {
    if rec.style.output == "-" {
        bail!("rec cannot write the SVG to stdout (the session runs there); pass -o <file>");
    }
    let theme = theme::builtin::load(&rec.style.theme)?;
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
    render_cast(header, &events, &theme, &rec.style, &rec.anim)
}

/// The v1 path: render one screen to a static SVG and write it out.
fn render_static(
    mut screen: Screen,
    title: Option<String>,
    theme: &theme::Theme,
    style: &StyleArgs,
) -> Result<()> {
    let (font_family, font_faces) = if style.no_font_embed {
        let family = style
            .font_family
            .clone()
            .unwrap_or_else(|| render::DEFAULT_FONT_STACK.to_string());
        (family, None)
    } else {
        let covered = font::subset::coverage(font::assets::regular())?;
        screen.split_uncovered(&covered);
        let faces = embedded_faces(std::iter::once(&screen), title.as_deref())?;
        (font::EMBEDDED_FONT_STACK.to_string(), faces)
    };

    let config = render_config(style, title, font_family, font_faces);
    let svg = render::render(&screen, theme, &config)?;
    write_output(&style.output, &svg)
}

/// Render an asciicast to an animated SVG (or its final screen with
/// --static).
fn render_cast(
    header: cast::Header,
    events: &[cast::Event],
    theme: &theme::Theme,
    style: &StyleArgs,
    anim_args: &AnimArgs,
) -> Result<()> {
    let title = style.title.clone().or(header.title.clone());

    if anim_args.static_ {
        // Final state only: concatenate the output and reuse the v1 path
        // (scrollback included, trailing blank rows trimmed).
        let mut bytes = Vec::new();
        for event in events {
            if let cast::EventData::Output(data) = &event.data {
                bytes.extend_from_slice(data.as_bytes());
            }
        }
        let screen = term::interpret(&bytes, header.width, header.height, theme);
        return render_static(screen, title, theme, style);
    }

    let opts = anim::AnimOptions {
        idle_time_limit: anim_args.idle_time_limit,
        speed: anim_args.speed,
    };
    let mut animation = anim::build_frames(&header, events, theme, &opts);

    let (font_family, font_faces) = if style.no_font_embed {
        let family = style
            .font_family
            .clone()
            .unwrap_or_else(|| render::DEFAULT_FONT_STACK.to_string());
        (family, None)
    } else {
        let covered = font::subset::coverage(font::assets::regular())?;
        for frame in &mut animation.frames {
            frame.screen.split_uncovered(&covered);
        }
        let faces = embedded_faces(animation.frames.iter().map(|f| &f.screen), title.as_deref())?;
        (font::EMBEDDED_FONT_STACK.to_string(), faces)
    };

    let config = render_config(style, title, font_family, font_faces);
    let svg = render::render_animated(&animation, theme, &config, !anim_args.no_loop)?;
    write_output(&style.output, &svg)
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
        window: !style.no_window,
        shadow: !style.no_shadow,
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
/// subset into the bold face, everything else — including the title — into
/// regular) and build the base64 WOFF2 @font-face payloads.
fn embedded_faces<'a>(
    screens: impl Iterator<Item = &'a Screen>,
    title: Option<&str>,
) -> Result<Option<render::FontFaces>> {
    use std::collections::BTreeSet;

    let mut regular: BTreeSet<char> = title.map(|t| t.chars().collect()).unwrap_or_default();
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
