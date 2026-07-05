# terminal-svg

Pixel-perfect SVG screenshots of terminal output, from a single self-contained binary.

![terminal-svg rendering a starship prompt and cargo build output](docs/hero.svg)

Point it at a command, a file, or a pipe and it produces an SVG with macOS-style
window chrome, your favourite colour scheme, and — the part that makes it
"perfect" — a **subsetted JetBrains Mono Nerd Font embedded in the SVG itself**,
so it renders identically everywhere: GitHub READMEs, blog posts, browsers on
machines with no fonts installed. The image above is terminal-svg's own output.

## Why another one?

Tools in this space usually either regex-strip ANSI codes (breaking progress
bars and cursor movement) or reference system fonts (breaking alignment on
every machine that doesn't have the font). terminal-svg does neither:

- **Real terminal emulation** — output is fed through a proper VT state
  machine ([avt](https://github.com/asciinema/avt), the engine behind
  asciinema). Carriage-return progress bars, `ESC[K` clears, and cursor-up
  repaints all resolve to exactly the final screen a real terminal would show.
- **Per-screenshot font subsetting** — only the glyphs actually used are
  embedded (as WOFF2), typically adding just a few KB. Box-drawing characters
  align seamlessly, Nerd Font powerline segments just work, and wide CJK
  characters occupy exactly two cells.
- **Emoji stay emoji** — colour emoji can't live in a monochrome font subset,
  so they're emitted as their own explicitly-positioned text runs and render
  through the viewer's native emoji font without knocking later columns out
  of alignment.

## Usage

```sh
# Run a command in a PTY (it sees a real TTY, so colours are on)
terminal-svg -- lsd -la
terminal-svg --title "tests" -o tests.svg -- cargo test

# Pipe ANSI output through it
ls --color=always | terminal-svg -o ls.svg

# Render a captured ANSI dump
terminal-svg dump.ansi -t nord -o dump.svg
```

## Recording animated SVGs

`terminal-svg rec` drops you into your shell, records everything, and renders
an **animated SVG** replaying the session when you exit — a GIF-quality demo
at a fraction of the size, with real selectable text, that plays anywhere an
`<img>` tag does (GitHub READMEs included, no JavaScript).

```sh
# Record an interactive session ($SHELL); exit the shell to finish
terminal-svg rec -o demo.svg

# Record a single command instead of a shell
terminal-svg rec -o build.svg -- cargo build

# Tweak the replay without re-recording: rec keeps an asciicast next to
# the SVG (demo.cast above), and .cast files render directly
terminal-svg demo.cast -t github-dark -o demo-dark.svg
terminal-svg demo.cast --speed 2 --no-loop -o demo-fast.svg
```

Recordings are standard [asciicast v2](https://docs.asciinema.org/manual/asciicast/v2/)
files — `asciinema play demo.cast` works, and existing asciinema recordings
render with plain `terminal-svg session.cast`. Long pauses are capped at 2s
(`--idle-time-limit`), bursts are coalesced to ≤30fps, identical frames are
deduplicated, and repeated rows are shared across frames via `<defs>`/`<use>`,
so even minute-long sessions stay compact. The animation loops with a 1.5s
hold on the last frame; `--no-loop` plays once and freezes, and `--static`
renders just the final screen.

### Options

| Flag | Default | |
|---|---|---|
| `-o, --output` | `terminal.svg` | `-` writes to stdout |
| `-t, --theme` | `dracula` | built-in name or path to a `.toml` |
| `--title` | command string | title bar text |
| `-c, --cols` / `-r, --rows` | 80 × 24 | PTY size; image height follows content |
| `--font-size` / `--line-height` | 14 / 1.2 | |
| `--padding` / `--margin` | 16 / 24 | margin is 0 with `--no-shadow` |
| `--no-window` | | bare rounded panel, no chrome |
| `--no-shadow` | | |
| `--no-font-embed` | | reference system fonts instead |
| `--timeout <secs>` | | kill the PTY command after N seconds |
| `--list-themes` | | |

Animation options (for `rec` and `.cast` input):

| Flag | Default | |
|---|---|---|
| `--idle-time-limit <secs>` | 2 | cap pauses between events |
| `--speed <n>` | 1 | playback speed multiplier |
| `--no-loop` | | play once and hold the last frame |
| `--static` | | render only the final screen |
| `--cast <path>` | output stem + `.cast` | where `rec` saves the recording |
| `-c` / `-r` (rec) | current terminal size | recorded PTY size |

### Themes

`dracula` (default), `catppuccin-mocha`, `nord`, `tokyo-night`, `github-dark`,
`github-light`, `solarized-dark`.

![SGR styles in catppuccin-mocha](docs/styles-catppuccin.svg)
![box drawing in github-light](docs/boxes-light.svg)

Custom themes are a small TOML file (16 ANSI colours + foreground/background,
optional chrome overrides) — see the [theme format reference](docs/themes.md):

```sh
terminal-svg -t my-theme.toml -- htop
```

## Installing

```sh
brew install russmckendrick/tap/terminal-svg
```

Or grab a binary for macOS/Linux/Windows from the
[releases page](https://github.com/russmckendrick/terminal-svg/releases).

## Building

```sh
cargo build --release
```

The JetBrainsMono Nerd Font Mono faces in [assets/fonts/](assets/fonts/)
(SIL OFL) are compressed at build time and baked into the binary — no runtime
dependencies, nothing to install.

Development loop:

```sh
cargo test                      # unit + golden + PTY integration tests
UPDATE_GOLDEN=1 cargo test      # refresh golden SVGs after rendering changes
./examples/gallery.sh           # render all fixtures × themes → gallery.html
```

## How it works

The short version below; the full walkthrough (including the two
renderer-compatibility rules that keep columns aligned everywhere) is in
[docs/architecture.md](docs/architecture.md).

1. **Capture** — spawn the command in a pseudo-terminal
   ([portable-pty](https://crates.io/crates/portable-pty)) or read bytes from
   stdin/file.
2. **Interpret** — feed everything through avt; read back the final grid
   (scrollback + screen), resolve inverse/faint/palette colours, and merge
   adjacent same-style cells into runs.
3. **Render** — lay the grid out with metrics read from the actual bundled
   font (ttf-parser), draw background rects, text runs, and decoration lines
   (CSS `text-decoration` is unreliable across SVG renderers, so underlines
   are real `<line>` elements), wrap it in window chrome.
4. **Embed** — collect the glyphs used per weight, subset with
   [allsorts](https://crates.io/crates/allsorts) (keeping a Unicode cmap —
   browsers reject fonts without one), encode to WOFF2
   ([ttf2woff2](https://crates.io/crates/ttf2woff2), pure Rust), and inline
   as a base64 `@font-face`.

## License

MIT. Bundled fonts are licensed under the SIL Open Font License —
see [assets/fonts/OFL.txt](assets/fonts/OFL.txt).
