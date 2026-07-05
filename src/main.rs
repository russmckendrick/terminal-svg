use anyhow::Result;
use clap::Parser;

use terminal_svg::cli::Cli;
use terminal_svg::{capture, font, render, term, theme};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_themes {
        for name in theme::builtin::names() {
            println!("{name}");
        }
        return Ok(());
    }

    let theme = theme::builtin::load(&cli.theme)?;

    let source = if !cli.command.is_empty() {
        capture::Source::Command(cli.command.clone())
    } else if let Some(path) = &cli.input {
        capture::Source::File(path.clone())
    } else {
        capture::Source::Stdin
    };
    let captured = capture::capture(source, cli.cols as u16, cli.rows as u16, cli.timeout)?;

    let mut screen = term::interpret(&captured.bytes, cli.cols, cli.rows, &theme);

    let title = cli.title.clone().or(captured.title);
    let (font_family, font_faces) = if cli.no_font_embed {
        let family = cli
            .font_family
            .clone()
            .unwrap_or_else(|| render::DEFAULT_FONT_STACK.to_string());
        (family, None)
    } else {
        let covered = font::subset::coverage(font::assets::regular())?;
        screen.split_uncovered(&covered);
        let faces = embedded_faces(&screen, title.as_deref())?;
        (font::EMBEDDED_FONT_STACK.to_string(), faces)
    };

    let config = render::RenderConfig {
        font_size: cli.font_size,
        line_height: cli.line_height,
        padding: cli.padding,
        margin: cli.margin(),
        window: !cli.no_window,
        shadow: !cli.no_shadow,
        title,
        font_family,
        font_faces,
    };

    let svg = render::render(&screen, &theme, &config)?;

    if cli.output == "-" {
        print!("{svg}");
    } else {
        std::fs::write(&cli.output, &svg)?;
        eprintln!("wrote {}", cli.output);
    }
    Ok(())
}

/// Collect the chars each face must cover (bold runs subset into the bold
/// face, everything else — including the title — into regular) and build
/// the base64 WOFF2 @font-face payloads.
fn embedded_faces(
    screen: &term::screen::Screen,
    title: Option<&str>,
) -> Result<Option<render::FontFaces>> {
    use std::collections::BTreeSet;

    let mut regular: BTreeSet<char> = title.map(|t| t.chars().collect()).unwrap_or_default();
    let mut bold: BTreeSet<char> = BTreeSet::new();
    for run in screen.rows.iter().flatten() {
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
