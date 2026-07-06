# How terminal-svg works

The pipeline is four stages, each a module that only talks to the next:

```
capture/          term/              render/            font/
bytes from  ──▶   avt VT engine ──▶  grid → SVG    ◀──  subset → WOFF2
PTY | stdin       final screen       chrome, text       base64 @font-face
| file            as styled runs     rects, lines
```

## 1. Capture (`src/capture/`)

Three sources produce the same thing — raw bytes:

- **PTY** (`pty.rs`): the command is spawned under a pseudo-terminal via
  `portable-pty`, so `isatty()` is true and programs emit colour and
  interactive output. `TERM=xterm-256color` and `COLORTERM=truecolor` are
  set. A reader thread drains the master side; the main thread waits on the
  child with an optional timeout (kill + keep whatever was captured).
- **stdin / file** (`input.rs`): read to end, nothing clever.

## 2. Interpret (`src/term/`)

The bytes are fed through [avt](https://github.com/asciinema/avt),
asciinema's virtual terminal. This is the fidelity core: carriage-return
progress bars, `ESC[K` clears, cursor-up repaints, scroll regions and wide
characters all resolve exactly as a real terminal would resolve them. We
read back `lines()` — scrollback plus visible screen — so output height
follows content rather than the terminal size.

One wrinkle: a VT treats `\n` as strict "move down" (index). On a real
terminal it's the tty driver's ONLCR that turns program `\n` into `\r\n`.
PTY captures already contain `\r\n`; piped/file input has bare `\n`, so
`normalize_newlines` emulates the line discipline (idempotently).

Each grid row is collapsed into **styled runs** (`screen.rs`): maximal
horizontal spans sharing identical *resolved* attributes. Resolution happens
here, not in the SVG layer — inverse swaps fg/bg, faint blends fg 50% toward
the effective bg, indexed colours become concrete RGB via the theme, blink
renders static. Wide (2-cell) characters always get their own run.

## 3. Render (`src/render/`)

Layout metrics come from the actual bundled font via `ttf-parser`
(`metrics.rs`): cell width from the 'M' advance, baseline from
ascender/descender centred in the line box, underline/strikeout position
and thickness from the font's `post`/`OS/2` tables. Because the same font
is embedded in the output, geometry and glyphs can't disagree.

The document (`svg.rs`) is assembled with plain `write!` templating in
layers:

1. `feDropShadow` filter + window body `<rect rx="10">` (`chrome.rs`),
   traffic lights, centred title;
2. background rects — one per run with a non-default background, exactly
   `line_h` tall so rows tile without seams;
3. text runs — one `<text x y>` per space-free segment (`text.rs`);
4. underline/strikethrough as real `<line>` elements (CSS
   `text-decoration` is unreliable across SVG renderers).

Two renderer-compatibility rules are load-bearing:

- **No space glyphs, ever.** Chrome ignores `xml:space="preserve"` and
  collapses whitespace, which would shift columns. Every run is split into
  space-free segments, each with an explicit `x`. Spaces carry no visual
  information anyway — backgrounds and decorations are separate elements.
- **Uncovered characters get isolated.** Anything the bundled font can't
  render (emoji) is split into its own single-char run before rendering, so
  the viewer's fallback font can advance however it likes without pushing
  later columns out of alignment.

## 4. Embed (`src/font/`)

`build.rs` deflate-compresses the vendored JetBrainsMono Nerd Font Mono
faces (Regular + Bold) into `OUT_DIR`; `assets.rs` bakes them into the
binary and decompresses once at runtime (~5 MB binary instead of ~7).

Per screenshot, `subset.rs` collects the characters each weight actually
uses (bold runs → Bold face, everything else + the title → Regular),
subsets with `allsorts`, encodes WOFF2 with `ttf2woff2` (pure Rust), and
inlines the result as base64 `@font-face` blocks. Typical cost: a few KB.

Two constraints discovered the hard way:

- The subsetter must **keep a Unicode cmap** — `allsorts` with
  `CmapTarget::Unicode` does; typst's `subsetter` crate strips cmap
  entirely and browsers reject the font.
- Emoji are deliberately **not** subset: JetBrains Mono has no colour
  emoji, and monochrome subsets can't carry them. The font-family chain
  ends in `Apple Color Emoji / Segoe UI Emoji / Noto Color Emoji` so they
  render natively everywhere.

## Testing

- **Unit tests** live next to the code (theme parsing, 256-colour cube math,
  run merging, VT edge cases like cursor-up redraws).
- **Golden tests** (`tests/golden.rs`) render every fixture in
  `tests/fixtures/*.ansi` with a fixed config and string-compare against
  `tests/golden/*.svg`. Rendering changes are reviewed as golden diffs:
  `UPDATE_GOLDEN=1 cargo test --test golden`.
- **PTY integration tests** (`tests/pty.rs`) spawn real commands and assert
  on captured colour and timeout enforcement.
- **Visual sweep**: `./scripts/gallery.sh` renders all fixtures × themes
  into `gallery.html`. For spot checks use headless Chrome — Quick Look
  does not load embedded WOFF2.
