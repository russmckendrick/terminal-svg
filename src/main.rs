use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches};

use terminal_svg::cli::{Cli, Sub};
use terminal_svg::embed::{EmbeddedSource, SourceKind};
use terminal_svg::pipeline::{SourceInput, render_svg};
use terminal_svg::{capture, cast, config, embed, theme};

fn main() -> Result<()> {
    let matches = Cli::command().get_matches();
    let mut cli = Cli::from_arg_matches(&matches)?;

    if let Some(shell) = cli.completions {
        clap_complete::generate(
            shell,
            &mut Cli::command(),
            "terminal-svg",
            &mut std::io::stdout(),
        );
        return Ok(());
    }
    if cli.man {
        clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout())?;
        return Ok(());
    }

    let file_config = config::load(cli.config.as_deref())?;
    config::apply(&mut cli, &matches, &file_config)?;

    if let Some(sub) = cli.sub.take() {
        return match sub {
            Sub::Rec(rec) => run_rec(&rec),
            Sub::Extract(args) => run_extract(&args),
            Sub::Edit(args) => run_edit(&args),
            Sub::Editor(mut args) => run_editor(&mut args, &matches),
        };
    }

    if cli.list_themes {
        for name in theme::builtin::names() {
            println!("{name}");
        }
        return Ok(());
    }

    // A terminal-svg SVG is its own source: recover the embedded
    // recording and re-render it, letting flags override the options it
    // was rendered with.
    if let Some(path) = cli
        .input
        .clone()
        .filter(|p| embed::looks_like_terminal_svg(p))
    {
        let svg_text = std::fs::read_to_string(&path)?;
        let source = embed::extract(&svg_text)?
            .with_context(|| format!("{} has no embedded terminal-svg source", path.display()))?;
        config::apply_embedded(&mut cli, &matches, &source.options)?;
        return match source.kind {
            SourceKind::Cast => {
                let (header, events) = cast::parse(&source.data[..])?;
                render_cast_input(&cli, header, events, source.data)
            }
            SourceKind::Ansi => {
                let title_fallback = source.options.title_fallback.clone();
                render_ansi_input(&cli, source.data, title_fallback)
            }
        };
    }

    if let Some(path) = cli.input.as_deref().filter(|p| cast::looks_like_cast(p)) {
        let bytes = std::fs::read(path)?;
        let (header, events) = cast::parse(&bytes[..])?;
        return render_cast_input(&cli, header, events, bytes);
    }

    let source = if !cli.command.is_empty() {
        capture::Source::Command(cli.command.clone())
    } else if let Some(path) = &cli.input {
        capture::Source::File(path.clone())
    } else {
        capture::Source::Stdin
    };
    let captured = capture::capture(source, cli.cols as u16, cli.rows as u16, cli.timeout)?;
    render_ansi_input(&cli, captured.bytes, captured.title)
}

/// Render a parsed cast and write the SVG, embedding the cast file bytes.
fn render_cast_input(
    cli: &Cli,
    header: cast::Header,
    events: Vec<cast::Event>,
    cast_bytes: Vec<u8>,
) -> Result<()> {
    let opts = cli.style.to_options(&cli.anim);
    let svg = render_svg(
        &SourceInput::Cast {
            header: &header,
            events: &events,
        },
        &opts,
    )?;
    let source = EmbeddedSource {
        kind: SourceKind::Cast,
        data: cast_bytes,
        options: opts,
    };
    write_output(&cli.style, &svg, Some(&source))
}

/// Render captured/recovered ANSI bytes and write the SVG, embedding them.
fn render_ansi_input(cli: &Cli, bytes: Vec<u8>, title_fallback: Option<String>) -> Result<()> {
    let mut opts = cli.style.to_options(&cli.anim);
    opts.cols = Some(cli.cols);
    opts.rows = Some(cli.rows);
    opts.title_fallback = title_fallback;
    let svg = render_svg(&SourceInput::Ansi { bytes: &bytes }, &opts)?;
    let source = EmbeddedSource {
        kind: SourceKind::Ansi,
        data: bytes,
        options: opts,
    };
    write_output(&cli.style, &svg, Some(&source))
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
    let cast_bytes = std::fs::read(&cast_path)?;
    let (header, events) = cast::parse(&cast_bytes[..])?;
    let opts = rec.style.to_options(&rec.anim);
    let svg = render_svg(
        &SourceInput::Cast {
            header: &header,
            events: &events,
        },
        &opts,
    )?;
    let source = EmbeddedSource {
        kind: SourceKind::Cast,
        data: cast_bytes,
        options: opts,
    };
    write_output(&rec.style, &svg, Some(&source))
}

/// `terminal-svg editor`: serve the visual editor on localhost and open
/// the browser. Runs until interrupted.
fn run_editor(args: &mut terminal_svg::cli::EditorArgs, matches: &clap::ArgMatches) -> Result<()> {
    use terminal_svg::editor;

    let (source, name) = match &args.input {
        Some(path) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            let data = std::fs::read(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let text = String::from_utf8_lossy(&data);
            let head = text.trim_start();

            let source = if head.starts_with("<?xml") || head.starts_with("<svg") {
                // A terminal-svg SVG seeds the controls with the options
                // it carries; flags on the editor invocation still win.
                let embedded = embed::extract(&text)?.with_context(|| {
                    format!("{} has no embedded terminal-svg source", path.display())
                })?;
                let m = matches
                    .subcommand_matches("editor")
                    .expect("editor subcommand parsed");
                config::apply_embedded_style(
                    &mut args.style,
                    &mut args.anim,
                    m,
                    &embedded.options,
                )?;
                let mut opts = args.style.to_options(&args.anim);
                // The editor has no --cols/--rows; capture context carries over.
                opts.cols = embedded.options.cols;
                opts.rows = embedded.options.rows;
                opts.title_fallback = embedded.options.title_fallback.clone();
                EmbeddedSource {
                    kind: embedded.kind,
                    data: embedded.data,
                    options: opts,
                }
            } else {
                editor::sniff_source(&name, data, args.style.to_options(&args.anim))?
            };
            (Some(source), Some(name))
        }
        None => (None, None),
    };

    let editor = editor::Editor::new(
        source,
        name,
        args.style.output.clone(),
        args.style.to_options(&args.anim),
    );
    editor::serve(&editor, args.port, |url| {
        eprintln!("terminal-svg editor listening on {url} (ctrl-c to stop)");
        if !args.no_open {
            open_browser(url);
        }
    })
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let launcher = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let launcher = "xdg-open";
    #[cfg(windows)]
    let launcher = "explorer";
    if let Err(e) = std::process::Command::new(launcher).arg(url).spawn() {
        eprintln!("could not open the browser ({e}); visit {url}");
    }
}

/// `terminal-svg edit`: clean up a recording and write it back out in
/// the same asciicast version.
fn run_edit(args: &terminal_svg::cli::EditArgs) -> Result<()> {
    use terminal_svg::edit;

    if args.redact.is_empty() && args.cut.is_empty() && args.max_pause.is_none() {
        bail!("nothing to do: pass --redact, --cut, and/or --max-pause");
    }
    if args.output != "-" && std::path::Path::new(&args.output) == args.input {
        bail!("refusing to edit in place; write to a new file (or - for stdout)");
    }

    let (mut header, mut events) = cast::read(&args.input)?;
    let stats = edit::apply(
        &mut header,
        &mut events,
        &edit::EditOps {
            redact: args.redact.clone(),
            cuts: args.cut.clone(),
            max_pause: args.max_pause,
        },
    )?;

    if args.output == "-" {
        let stdout = std::io::stdout();
        cast::write(stdout.lock(), &header, &events)?;
    } else {
        let file = std::fs::File::create(&args.output)
            .with_context(|| format!("failed to create {}", args.output))?;
        cast::write(std::io::BufWriter::new(file), &header, &events)?;
        eprintln!(
            "wrote {} ({} masked, {} events cut, {:.1}s removed)",
            args.output, stats.redactions, stats.events_cut, stats.time_removed
        );
    }
    Ok(())
}

/// `terminal-svg extract`: recover the source a terminal-svg SVG carries.
fn run_extract(args: &terminal_svg::cli::ExtractArgs) -> Result<()> {
    let svg_text = std::fs::read_to_string(&args.input)
        .with_context(|| format!("failed to read {}", args.input.display()))?;
    let source = embed::extract(&svg_text)?.with_context(|| {
        format!(
            "{} has no embedded terminal-svg source (rendered with \
             --no-embed-source, or stripped by an SVG optimizer?)",
            args.input.display()
        )
    })?;
    match &args.output {
        Some(path) => {
            std::fs::write(path, &source.data)?;
            eprintln!(
                "wrote {} ({} source)",
                path.display(),
                source.kind.extension()
            );
        }
        None => {
            use std::io::Write;
            std::io::stdout().write_all(&source.data)?;
        }
    }
    Ok(())
}

fn write_output(
    style: &terminal_svg::cli::StyleArgs,
    svg: &str,
    source: Option<&EmbeddedSource>,
) -> Result<()> {
    let document = match source.filter(|_| !style.no_embed_source) {
        Some(s) => embed::embed(svg, s),
        None => svg.to_string(),
    };
    if style.output == "-" {
        print!("{document}");
    } else {
        std::fs::write(&style.output, document)?;
        eprintln!("wrote {}", style.output);
    }
    Ok(())
}
