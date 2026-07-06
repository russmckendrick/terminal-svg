//! Golden-file SVG tests: each fixture renders with a fixed theme and no
//! font embedding (keeps diffs reviewable), then string-compares against
//! tests/golden/<name>.svg.
//!
//! Regenerate after intentional rendering changes with:
//!   UPDATE_GOLDEN=1 cargo test --test golden

use std::path::Path;

use terminal_svg::render::{self, RenderConfig};
use terminal_svg::{anim, cast, term, theme};

const FIXTURES: &[&str] = &[
    "colors16",
    "colors256",
    "truecolor",
    "sgr-styles",
    "boxdrawing",
    "progress",
    "cjk-emoji",
    "starship",
];

fn fixed_config(title: &str) -> RenderConfig {
    RenderConfig {
        font_size: 14.0,
        line_height: 1.2,
        padding: 16.0,
        margin: 24.0,
        chrome: render::ChromeStyle::Macos,
        background: true,
        shadow: true,
        title: Some(title.to_string()),
        font_family: render::DEFAULT_FONT_STACK.to_string(),
        font_faces: None,
        cursor: render::CursorStyle::Block,
    }
}

fn render_fixture(name: &str) -> String {
    let bytes = std::fs::read(format!("tests/fixtures/{name}.ansi"))
        .unwrap_or_else(|e| panic!("fixture {name} missing: {e}"));
    let theme = theme::builtin::load("dracula").unwrap();
    let screen = term::interpret(&bytes, 80, 24);
    render::render(&screen, &theme, &fixed_config(name)).unwrap()
}

/// The colors16 fixture rendered with a non-default chrome, locking the
/// windows/ubuntu title-bar markup.
fn render_chrome_fixture(chrome: render::ChromeStyle, golden_name: &str) -> String {
    let bytes = std::fs::read("tests/fixtures/colors16.ansi").unwrap();
    let theme = theme::builtin::load("dracula").unwrap();
    let screen = term::interpret(&bytes, 80, 24);
    let config = RenderConfig {
        chrome,
        ..fixed_config(golden_name)
    };
    render::render(&screen, &theme, &config).unwrap()
}

fn default_anim_opts() -> anim::AnimOptions {
    anim::AnimOptions {
        idle_time_limit: None,
        speed: 1.0,
        from: None,
        to: None,
    }
}

/// Animated rendering is deterministic: the pipeline consumes only the
/// cast's own timestamps, never the wall clock.
fn render_animated_fixture(name: &str, title: &str, opts: &anim::AnimOptions) -> String {
    let (header, events) = cast::read(Path::new(&format!("tests/fixtures/{name}.cast")))
        .unwrap_or_else(|e| panic!("fixture {name} missing: {e}"));
    let theme = theme::builtin::load("dracula").unwrap();
    let animation = anim::build_frames(&header, &events, opts);
    render::render_animated(&animation, &theme, &fixed_config(title), true).unwrap()
}

/// The typing recording as a dual light/dark animated document, locking
/// the shared-frames + palette-CSS markup.
fn render_dual_animated_fixture() -> String {
    let (header, events) = cast::read(Path::new("tests/fixtures/typing.cast")).unwrap();
    let animation = anim::build_frames(&header, &events, &default_anim_opts());
    let light = theme::builtin::load("github-light").unwrap();
    let dark = theme::builtin::load("github-dark").unwrap();
    render::render_animated_dual(&animation, &light, &dark, &fixed_config("dual-anim"), true)
        .unwrap()
}

#[test]
fn golden() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    let animated = (
        "typing",
        render_animated_fixture("typing", "typing", &default_anim_opts()),
    );
    // typing-v3.cast is the same recording hand-converted to asciicast v3;
    // v3 support is pure input normalization, so the SVGs must match
    // byte-for-byte.
    assert_eq!(
        render_animated_fixture("typing-v3", "typing", &default_anim_opts()),
        animated.1,
        "typing-v3.cast must render identical to typing.cast"
    );
    // A --from/--to window into the same recording: seeds the typed prompt,
    // animates the progress bar, cuts before the resize and final output.
    let trimmed = (
        "typing-trimmed",
        render_animated_fixture(
            "typing",
            "typing-trimmed",
            &anim::AnimOptions {
                from: Some(1.0),
                to: Some(6.5),
                ..default_anim_opts()
            },
        ),
    );
    let rendered = FIXTURES
        .iter()
        .map(|name| (*name, render_fixture(name)))
        .chain([
            (animated.0, animated.1),
            trimmed,
            ("dual-anim", render_dual_animated_fixture()),
            (
                "chrome-windows",
                render_chrome_fixture(render::ChromeStyle::Windows, "chrome-windows"),
            ),
            (
                "chrome-ubuntu",
                render_chrome_fixture(render::ChromeStyle::Ubuntu, "chrome-ubuntu"),
            ),
        ]);

    for (name, svg) in rendered {
        let golden_path = format!("tests/golden/{name}.svg");

        if update {
            std::fs::write(&golden_path, &svg).unwrap();
            continue;
        }

        if !Path::new(&golden_path).exists() {
            failures.push(format!("{name}: golden file missing — run UPDATE_GOLDEN=1"));
            continue;
        }
        let expected = std::fs::read_to_string(&golden_path).unwrap();
        if svg != expected {
            failures.push(format!(
                "{name}: output differs from {golden_path} — \
                 rerun with UPDATE_GOLDEN=1 if the change is intentional"
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
