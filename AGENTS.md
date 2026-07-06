# AGENTS.md

## Tooling

- Tests: `cargo test` (unit + golden + PTY integration)
- Golden SVGs: after an intentional rendering change, regenerate with `UPDATE_GOLDEN=1 cargo test --test golden` and review the diff — a bare `cargo test` failure in `golden` usually means you changed output, not that you broke something
- Lint bar: `cargo clippy --all-targets -- -D warnings && cargo fmt --check`
- Visual sweep: `./scripts/gallery.sh && open gallery.html` (all fixtures × themes)
- Help output: after any CLI change (new flag, help text, defaults), review `cargo run -- -h`, `cargo run -- --help`, and `cargo run -- rec -h` before committing. Every flag needs a `help_heading` matching the section tables in `docs/usage.md` (Output & themes / Window / Layout & fonts / Capture / Animation) and a human `value_name` (`<PX>`, `<SECONDS>`, `<STYLE>` — never a leaked field name like `<FONT_SIZE>`). Examples live in the `EXAMPLES` `after_help` string in `src/cli.rs`, not in doc comments — clap collapses doc-comment newlines, so multi-line examples there turn into an unreadable run-on line. The man page (`--man`) and completions are generated from the same clap definitions, so they inherit whatever the help says.

## Releasing

1. Bump `version` in `Cargo.toml` and commit — CI fails the release if it doesn't match the tag.
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`. CI does the rest: checks on all three OSes, five release binaries + `.sha256` checksums, the GitHub release, the Homebrew tap bump, and a terminal-svg.dev redeploy.
3. **The release is created with GitHub's auto-generated notes as a placeholder — always replace them** (`gh release edit vX.Y.Z --title ... --notes-file ...`). Match the house style of v0.1.0/v0.2.0:
   - Title: `vX.Y.Z — <hook>`, not a bare version number.
   - An opening paragraph selling the headline feature, plus a showcase image via a `raw.githubusercontent.com` URL **pinned to the tag** (never `main` — files move).
   - `## Highlights` with bold lead-ins; a compatibility note if output changed.
   - `## Install` (brew + "grab a binary below" with the platform list), `## Quick start`, the docs links line, the OFL/MIT line, and the `**Full Changelog**` compare link.

## Non-Obvious Rules

- Never emit space characters inside SVG `<text>` elements — Chrome ignores `xml:space="preserve"` and collapses them, shifting terminal columns. Runs are split into space-free segments with explicit `x` (see `render/text.rs::space_free_segments`).
- Don't verify font rendering with `qlmanage`/Quick Look — it silently ignores embedded WOFF2. Use headless Chrome: `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless --screenshot=out.png --window-size=WxH file://.../out.svg`.
- Font subsetting must stay on `allsorts` with `CmapTarget::Unicode` — typst's `subsetter` crate strips the cmap table and browsers reject the result.
- Underline/strikethrough are drawn `<line>` elements on purpose; CSS `text-decoration` is inconsistent across SVG renderers.
- `tests/fixtures/*.ansi` are frozen; edit `scripts/make-fixtures.sh` and rerun it only when adding a fixture, then regenerate goldens.
- `assets/fonts/*.ttf` must exist before building (build.rs bakes them in); they're vendored, don't re-download unless updating the font version.
