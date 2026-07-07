//! Optional config file supplying personal defaults for CLI flags.
//!
//! Precedence: a flag given on the command line always wins; a key in the
//! config file beats the built-in default. The merge inspects clap's
//! `ValueSource` so `--help` keeps showing the real defaults.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::parser::ValueSource;

use crate::cli::{AnimArgs, Cli, StyleArgs, Sub};
use crate::render::{ChromeStyle, CursorStyle};

/// Keys mirror the long flag names.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub theme: Option<String>,
    pub theme_light: Option<String>,
    pub theme_dark: Option<String>,
    pub chrome: Option<ChromeStyle>,
    pub cursor: Option<CursorStyle>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub padding: Option<f32>,
    pub margin: Option<f32>,
    pub title_emoji: Option<String>,
    pub font_family: Option<String>,
    pub speed: Option<f64>,
    pub idle_time_limit: Option<f64>,
    pub no_shadow: Option<bool>,
    pub no_embed_source: Option<bool>,
}

/// The default config location: `$XDG_CONFIG_HOME/terminal-svg/config.toml`,
/// falling back to `~/.config/terminal-svg/config.toml`.
pub fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("terminal-svg").join("config.toml"))
}

/// Load an explicit `--config` path (must exist), or the default location
/// (silently absent = all defaults).
pub fn load(explicit: Option<&Path>) -> Result<Config> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => match default_path() {
            Some(p) if p.exists() => p,
            _ => return Ok(Config::default()),
        },
    };
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    parse(&source).with_context(|| format!("invalid config {}", path.display()))
}

fn parse(source: &str) -> Result<Config> {
    Ok(toml::from_str(source)?)
}

/// Overlay config values onto the parsed CLI, skipping anything the user
/// passed on the command line.
pub fn apply(cli: &mut Cli, matches: &clap::ArgMatches, config: &Config) -> Result<()> {
    match &mut cli.sub {
        Some(Sub::Rec(rec)) => {
            let m = matches
                .subcommand_matches("rec")
                .expect("rec subcommand parsed");
            apply_style(&mut rec.style, &mut rec.anim, m, config)
        }
        Some(Sub::Editor(editor)) => {
            let m = matches
                .subcommand_matches("editor")
                .expect("editor subcommand parsed");
            apply_style(&mut editor.style, &mut editor.anim, m, config)
        }
        // extract and edit have no render flags to default.
        Some(Sub::Extract(_) | Sub::Edit(_)) => Ok(()),
        None => apply_style(&mut cli.style, &mut cli.anim, matches, config),
    }
}

fn apply_style(
    style: &mut StyleArgs,
    anim: &mut AnimArgs,
    m: &clap::ArgMatches,
    c: &Config,
) -> Result<()> {
    // Clap arg ids are the snake_case field names.
    let from_cli = |id: &str| m.value_source(id) == Some(ValueSource::CommandLine);
    macro_rules! overlay {
        ($field:expr, $id:literal, $value:expr) => {
            if let Some(v) = &$value
                && !from_cli($id)
            {
                $field = v.clone().into();
            }
        };
    }

    overlay!(style.theme, "theme", c.theme);
    overlay!(style.theme_light, "theme_light", c.theme_light);
    overlay!(style.theme_dark, "theme_dark", c.theme_dark);
    overlay!(style.chrome, "chrome", c.chrome);
    overlay!(anim.cursor, "cursor", c.cursor);
    overlay!(style.font_size, "font_size", c.font_size);
    overlay!(style.line_height, "line_height", c.line_height);
    overlay!(style.padding, "padding", c.padding);
    overlay!(style.margin, "margin", c.margin);
    overlay!(style.title_emoji, "title_emoji", c.title_emoji);
    overlay!(style.font_family, "font_family", c.font_family);
    overlay!(style.no_shadow, "no_shadow", c.no_shadow);
    overlay!(anim.speed, "speed", c.speed);
    overlay!(anim.idle_time_limit, "idle_time_limit", c.idle_time_limit);
    overlay!(style.no_embed_source, "no_embed_source", c.no_embed_source);

    // Clap's `requires` pairing only sees command-line args; re-check after
    // the merge so a config half-pair still errors clearly.
    if style.theme_light.is_some() != style.theme_dark.is_some() {
        bail!("theme-light and theme-dark must be set together (across flags and config)");
    }
    Ok(())
}

/// Overlay the options embedded in a re-rendered SVG onto the parsed CLI,
/// skipping anything the user passed on the command line. Runs after
/// `apply`, so the resulting precedence is CLI > embedded > config file >
/// built-in defaults: the re-render reproduces the original document
/// except where flags say otherwise.
pub fn apply_embedded(
    cli: &mut Cli,
    matches: &clap::ArgMatches,
    o: &crate::options::RenderOptions,
) -> Result<()> {
    let from_cli = |id: &str| matches.value_source(id) == Some(ValueSource::CommandLine);
    if let Some(v) = o.cols
        && !from_cli("cols")
    {
        cli.cols = v;
    }
    if let Some(v) = o.rows
        && !from_cli("rows")
    {
        cli.rows = v;
    }
    apply_embedded_style(&mut cli.style, &mut cli.anim, matches, o)
}

/// The style/anim part of `apply_embedded`, shared with the `editor`
/// subcommand (whose ArgMatches has no cols/rows).
pub fn apply_embedded_style(
    style: &mut StyleArgs,
    anim: &mut AnimArgs,
    matches: &clap::ArgMatches,
    o: &crate::options::RenderOptions,
) -> Result<()> {
    let from_cli = |id: &str| matches.value_source(id) == Some(ValueSource::CommandLine);
    macro_rules! overlay {
        ($field:expr, $id:literal, $value:expr) => {
            if let Some(v) = &$value
                && !from_cli($id)
            {
                $field = v.clone().into();
            }
        };
    }
    // Embedded bools are effective values from the original render, not
    // "set or unset", so they copy over whenever the flag isn't on the
    // command line (a config no-shadow cannot undo an embedded shadow).
    macro_rules! overlay_flag {
        ($field:expr, $id:literal, $value:expr) => {
            if !from_cli($id) {
                $field = $value;
            }
        };
    }

    overlay!(style.theme, "theme", o.theme);
    overlay!(style.theme_light, "theme_light", o.theme_light);
    overlay!(style.theme_dark, "theme_dark", o.theme_dark);
    overlay!(style.chrome, "chrome", o.chrome);
    overlay_flag!(style.no_window, "no_window", o.no_window);
    overlay_flag!(style.no_background, "no_background", o.no_background);
    overlay_flag!(style.no_shadow, "no_shadow", o.no_shadow);
    overlay!(style.title, "title", o.title);
    overlay!(style.title_emoji, "title_emoji", o.title_emoji);
    overlay!(style.font_size, "font_size", o.font_size);
    overlay!(style.line_height, "line_height", o.line_height);
    overlay!(style.padding, "padding", o.padding);
    overlay!(style.margin, "margin", o.margin);
    overlay_flag!(style.no_font_embed, "no_font_embed", o.no_font_embed);
    overlay!(style.font_family, "font_family", o.font_family);
    overlay!(anim.cursor, "cursor", o.cursor);
    overlay!(anim.speed, "speed", o.speed);
    overlay!(anim.idle_time_limit, "idle_time_limit", o.idle_time_limit);
    overlay_flag!(anim.no_loop, "no_loop", o.no_loop);
    overlay!(anim.from, "from", o.from);
    overlay!(anim.to, "to", o.to);
    overlay_flag!(anim.static_, "static_", o.static_);
    overlay!(anim.at, "at", o.at);

    if style.theme_light.is_some() != style.theme_dark.is_some() {
        bail!(
            "theme-light and theme-dark must be set together (across flags and embedded options)"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches};

    fn merged(args: &[&str], toml: &str) -> Cli {
        let matches = Cli::command().try_get_matches_from(args).unwrap();
        let mut cli = Cli::from_arg_matches(&matches).unwrap();
        let config = parse(toml).unwrap();
        apply(&mut cli, &matches, &config).unwrap();
        cli
    }

    #[test]
    fn parses_full_config() {
        let c = parse(
            r#"
            theme = "nord"
            chrome = "ubuntu"
            cursor = "bar"
            font-size = 16.0
            no-shadow = true
            idle-time-limit = 3.0
            "#,
        )
        .unwrap();
        assert_eq!(c.theme.as_deref(), Some("nord"));
        assert_eq!(c.chrome, Some(ChromeStyle::Ubuntu));
        assert_eq!(c.cursor, Some(CursorStyle::Bar));
        assert_eq!(c.font_size, Some(16.0));
        assert_eq!(c.no_shadow, Some(true));
        assert_eq!(c.idle_time_limit, Some(3.0));
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(parse("font-szie = 16.0").is_err());
    }

    #[test]
    fn config_beats_default_cli_beats_config() {
        let toml = r#"
            theme = "nord"
            font-size = 16.0
            no-shadow = true
        "#;
        let cli = merged(&["terminal-svg"], toml);
        assert_eq!(cli.style.theme, "nord");
        assert_eq!(cli.style.font_size, 16.0);
        assert!(cli.style.no_shadow);

        let cli = merged(
            &["terminal-svg", "-t", "dracula", "--font-size", "12"],
            toml,
        );
        assert_eq!(cli.style.theme, "dracula");
        assert_eq!(cli.style.font_size, 12.0);
        assert!(cli.style.no_shadow); // untouched by CLI, config still applies
    }

    #[test]
    fn config_applies_to_rec_subcommand() {
        let cli = merged(&["terminal-svg", "rec"], "theme = \"nord\"\nspeed = 2.0");
        let Some(Sub::Rec(rec)) = cli.sub else {
            panic!("expected rec");
        };
        assert_eq!(rec.style.theme, "nord");
        assert_eq!(rec.anim.speed, 2.0);
    }

    #[test]
    fn half_paired_dual_theme_errors() {
        let matches = Cli::command()
            .try_get_matches_from(["terminal-svg"])
            .unwrap();
        let mut cli = Cli::from_arg_matches(&matches).unwrap();
        let config = parse("theme-light = \"github-light\"").unwrap();
        assert!(apply(&mut cli, &matches, &config).is_err());

        // The pair is fine when the other half comes from the CLI... but
        // clap's `requires` already rejects a lone --theme-dark, so the
        // valid spelling is config supplying both.
        let config = parse("theme-light = \"github-light\"\ntheme-dark = \"github-dark\"").unwrap();
        let mut cli = Cli::from_arg_matches(&matches).unwrap();
        apply(&mut cli, &matches, &config).unwrap();
        assert_eq!(
            cli.style.dual_themes(),
            Some(("github-light", "github-dark"))
        );
    }

    #[test]
    fn default_path_respects_xdg() {
        // Not parallel-safe to mutate env vars, so just check the shape.
        if let Some(p) = default_path() {
            assert!(p.ends_with("terminal-svg/config.toml"));
        }
    }
}
