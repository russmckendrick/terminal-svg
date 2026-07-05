//! Golden-file SVG tests: each fixture renders with a fixed theme and no
//! font embedding (keeps diffs reviewable), then string-compares against
//! tests/golden/<name>.svg.
//!
//! Regenerate after intentional rendering changes with:
//!   UPDATE_GOLDEN=1 cargo test --test golden

use std::path::Path;

use terminal_svg::render::{self, RenderConfig};
use terminal_svg::{term, theme};

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

fn render_fixture(name: &str) -> String {
    let bytes = std::fs::read(format!("tests/fixtures/{name}.ansi"))
        .unwrap_or_else(|e| panic!("fixture {name} missing: {e}"));
    let theme = theme::builtin::load("dracula").unwrap();
    let screen = term::interpret(&bytes, 80, 24, &theme);
    let config = RenderConfig {
        font_size: 14.0,
        line_height: 1.2,
        padding: 16.0,
        margin: 24.0,
        window: true,
        shadow: true,
        title: Some(name.to_string()),
        font_family: render::DEFAULT_FONT_STACK.to_string(),
        font_faces: None,
    };
    render::render(&screen, &theme, &config).unwrap()
}

#[test]
fn golden() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    for name in FIXTURES {
        let svg = render_fixture(name);
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
