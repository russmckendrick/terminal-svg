# AGENTS.md

## Tooling

- Tests: `cargo test` (unit + golden + PTY integration)
- Golden SVGs: after an intentional rendering change, regenerate with `UPDATE_GOLDEN=1 cargo test --test golden` and review the diff — a bare `cargo test` failure in `golden` usually means you changed output, not that you broke something
- Lint bar: `cargo clippy --all-targets -- -D warnings && cargo fmt --check`
- Visual sweep: `./scripts/gallery.sh && open gallery.html` (all fixtures × themes)
- Help output: after any CLI change (new flag, help text, defaults), review `cargo run -- -h`, `cargo run -- --help`, and `cargo run -- rec -h` before committing. Every flag needs a `help_heading` matching the section tables in `docs/usage.md` (Output & themes / Window / Layout & fonts / Capture / Animation) and a human `value_name` (`<PX>`, `<SECONDS>`, `<STYLE>` — never a leaked field name like `<FONT_SIZE>`). Examples live in the `EXAMPLES` `after_help` string in `src/cli.rs`, not in doc comments — clap collapses doc-comment newlines, so multi-line examples there turn into an unreadable run-on line. The man page (`--man`) and completions are generated from the same clap definitions, so they inherit whatever the help says.

## Releasing

1. **Write the release notes first**: `docs/releases/vX.Y.Z.md`, committed before tagging. CI validates it exists (and fails fast if not) and creates the release from it, so the chained site deploy — which bakes release notes in at build time — publishes the real notes on the first pass, no manual redeploy or after-the-fact `gh release edit`. Format:
   - First line is the title: `# vX.Y.Z — <hook>` (becomes the release title; never a bare version number). The rest of the file is the release body.
   - An opening paragraph selling the headline feature, plus a showcase image via a `raw.githubusercontent.com` URL **pinned to the tag** (never `main` — files move).
   - `## Highlights` with bold lead-ins; a compatibility note if output changed.
   - `## Install` (brew + "grab a binary below" with the platform list), `## Quick start`, the docs links line, the OFL/MIT line, and the `**Full Changelog**` compare link.
   - Match the house style of v0.5.0 (see `docs/releases/` or the published releases).
2. Bump `version` in `Cargo.toml` and commit — CI fails the release if it doesn't match the tag.
3. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`. CI does the rest: checks on all three OSes, five release binaries + `.sha256` checksums, the GitHub release (title + body from the notes file), the Homebrew tap bump, and a terminal-svg.dev redeploy.
   - The chained site deploy runs with the tag as its ref, so the `github-pages` environment (repo Settings → Environments) must allow tag deployments: it carries a `v*` tag policy alongside `main` (added 2026-07-06 via `gh api .../deployment-branch-policies -f name='v*' -f type=tag`). If the environment is ever recreated without it, every release run ends red with "not allowed to deploy to github-pages due to environment protection rules".
4. Fixing notes after the fact still works (`gh release edit` or edit the file and re-run), but remember the site bakes notes at build time — rerun the "🌐 Deploy Site" workflow (`gh workflow run deploy-site.yml`) after any post-release edit.

## Non-Obvious Rules

- Never emit space characters inside SVG `<text>` elements — Chrome ignores `xml:space="preserve"` and collapses them, shifting terminal columns. Runs are split into space-free segments with explicit `x` (see `render/text.rs::space_free_segments`).
- Don't verify font rendering with `qlmanage`/Quick Look — it silently ignores embedded WOFF2. Use headless Chrome: `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless --screenshot=out.png --window-size=WxH file://.../out.svg`.
- Font subsetting must stay on `allsorts` with `CmapTarget::Unicode` — typst's `subsetter` crate strips the cmap table and browsers reject the result.
- Underline/strikethrough are drawn `<line>` elements on purpose; CSS `text-decoration` is inconsistent across SVG renderers.
- `tests/fixtures/*.ansi` are frozen; edit `scripts/make-fixtures.sh` and rerun it only when adding a fixture, then regenerate goldens.
- `assets/fonts/*.ttf` must exist before building (build.rs bakes them in); they're vendored, don't re-download unless updating the font version.
