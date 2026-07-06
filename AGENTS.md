# AGENTS.md

## Tooling

- Tests: `cargo test` (unit + golden + PTY integration)
- Golden SVGs: after an intentional rendering change, regenerate with `UPDATE_GOLDEN=1 cargo test --test golden` and review the diff — a bare `cargo test` failure in `golden` usually means you changed output, not that you broke something
- Lint bar: `cargo clippy --all-targets -- -D warnings && cargo fmt --check`
- Visual sweep: `./scripts/gallery.sh && open gallery.html` (all fixtures × themes)

## Non-Obvious Rules

- Never emit space characters inside SVG `<text>` elements — Chrome ignores `xml:space="preserve"` and collapses them, shifting terminal columns. Runs are split into space-free segments with explicit `x` (see `render/text.rs::space_free_segments`).
- Don't verify font rendering with `qlmanage`/Quick Look — it silently ignores embedded WOFF2. Use headless Chrome: `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless --screenshot=out.png --window-size=WxH file://.../out.svg`.
- Font subsetting must stay on `allsorts` with `CmapTarget::Unicode` — typst's `subsetter` crate strips the cmap table and browsers reject the result.
- Underline/strikethrough are drawn `<line>` elements on purpose; CSS `text-decoration` is inconsistent across SVG renderers.
- `tests/fixtures/*.ansi` are frozen; edit `scripts/make-fixtures.sh` and rerun it only when adding a fixture, then regenerate goldens.
- `assets/fonts/*.ttf` must exist before building (build.rs bakes them in); they're vendored, don't re-download unless updating the font version.
