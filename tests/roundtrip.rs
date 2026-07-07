//! The SVG-is-its-own-source contract: an SVG rendered with embedding
//! carries enough to reproduce itself byte-for-byte and to recover the
//! original recording byte-for-byte.

use terminal_svg::embed::{self, EmbeddedSource, SourceKind};
use terminal_svg::options::RenderOptions;
use terminal_svg::pipeline::{SourceInput, render_svg};
use terminal_svg::{cast, config};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn render_embedded(cast_bytes: &[u8], options: &RenderOptions) -> String {
    let (header, events) = cast::parse(cast_bytes).unwrap();
    let svg = render_svg(
        &SourceInput::Cast {
            header: &header,
            events: &events,
        },
        options,
    )
    .unwrap();
    embed::embed(
        &svg,
        &EmbeddedSource {
            kind: SourceKind::Cast,
            data: cast_bytes.to_vec(),
            options: options.clone(),
        },
    )
}

#[test]
fn extract_recovers_the_cast_byte_exact() {
    let cast_bytes = fixture("typing.cast");
    let options = RenderOptions {
        theme: Some("dracula".into()),
        speed: Some(2.0),
        ..Default::default()
    };
    let embedded = render_embedded(&cast_bytes, &options);

    let source = embed::extract(&embedded).unwrap().expect("source embedded");
    assert_eq!(source.kind, SourceKind::Cast);
    assert_eq!(source.data, cast_bytes);
    assert_eq!(source.options, options);
}

#[test]
fn rerender_with_same_options_is_byte_identical() {
    let cast_bytes = fixture("typing.cast");
    let options = RenderOptions::default();
    let first = render_embedded(&cast_bytes, &options);

    let source = embed::extract(&first).unwrap().unwrap();
    let second = render_embedded(&source.data, &source.options);
    assert_eq!(first, second, "round-trip must be a fixed point");
}

#[test]
fn rerender_with_new_theme_differs_only_in_look_not_source() {
    let cast_bytes = fixture("typing.cast");
    let first = render_embedded(&cast_bytes, &RenderOptions::default());

    let source = embed::extract(&first).unwrap().unwrap();
    let retheme = RenderOptions {
        theme: Some("nord".into()),
        ..source.options.clone()
    };
    let second = render_embedded(&source.data, &retheme);

    assert_ne!(first, second);
    // The source survives another generation unchanged.
    let again = embed::extract(&second).unwrap().unwrap();
    assert_eq!(again.data, cast_bytes);
    assert_eq!(again.options.theme.as_deref(), Some("nord"));
}

#[test]
fn v3_casts_round_trip_too() {
    let cast_bytes = fixture("typing-v3.cast");
    let embedded = render_embedded(&cast_bytes, &RenderOptions::default());
    let source = embed::extract(&embedded).unwrap().unwrap();
    assert_eq!(source.data, cast_bytes);
}

#[test]
fn cli_beats_embedded_beats_config_beats_default() {
    use clap::{CommandFactory, FromArgMatches};
    use terminal_svg::cli::Cli;

    // Original render: font-size 16, nord, no-shadow.
    let embedded_options = RenderOptions {
        theme: Some("nord".into()),
        font_size: Some(16.0),
        no_shadow: true,
        speed: Some(1.0),
        ..Default::default()
    };

    // Re-render with `-t dracula` on the command line.
    let matches = Cli::command()
        .try_get_matches_from(["terminal-svg", "shot.svg", "-t", "dracula"])
        .unwrap();
    let mut cli = Cli::from_arg_matches(&matches).unwrap();
    config::apply_embedded(&mut cli, &matches, &embedded_options).unwrap();

    assert_eq!(cli.style.theme, "dracula"); // CLI wins
    assert_eq!(cli.style.font_size, 16.0); // embedded beats default
    assert!(cli.style.no_shadow); // embedded flag carries over

    // An embedded shadow=on (no_shadow: false) suppresses any config
    // default: the flag copies the original render faithfully.
    let matches = Cli::command()
        .try_get_matches_from(["terminal-svg", "shot.svg"])
        .unwrap();
    let mut cli = Cli::from_arg_matches(&matches).unwrap();
    cli.style.no_shadow = true; // pretend a config file set this
    config::apply_embedded(
        &mut cli,
        &matches,
        &RenderOptions {
            no_shadow: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!cli.style.no_shadow);
}

#[test]
fn half_paired_embedded_dual_theme_errors() {
    use clap::{CommandFactory, FromArgMatches};
    use terminal_svg::cli::Cli;

    let matches = Cli::command()
        .try_get_matches_from(["terminal-svg", "shot.svg"])
        .unwrap();
    let mut cli = Cli::from_arg_matches(&matches).unwrap();
    let broken = RenderOptions {
        theme_light: Some("github-light".into()),
        ..Default::default()
    };
    assert!(config::apply_embedded(&mut cli, &matches, &broken).is_err());
}
