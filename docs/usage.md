# CLI reference

```
terminal-svg [OPTIONS] [INPUT] [-- <COMMAND>...]
terminal-svg rec [OPTIONS] [-- <COMMAND>...]
```

## Input modes

terminal-svg takes terminal output from one of five places and always
produces an SVG:

**Run a command in a PTY.** Everything after `--` is spawned under a real
pseudo-terminal, so `isatty()` is true and programs switch on colour,
progress bars, and interactive layouts exactly as they would in your
terminal (`TERM=xterm-256color`, `COLORTERM=truecolor`):

```sh
terminal-svg -- lsd -la
terminal-svg --title "tests" -o tests.svg -- cargo test
```

**Pipe into stdin.** With no input file and no command, stdin is read to
end. Programs detect the pipe and usually strip colour, so force it on:

```sh
ls --color=always | terminal-svg -o ls.svg
```

**Render a file of captured ANSI.** Bytes are interpreted exactly as a
terminal would — carriage-return progress bars, `ESC[K` clears, and
cursor-up repaints all resolve to the final screen:

```sh
terminal-svg dump.ansi -t nord -o dump.svg
```

**Render an asciicast.** A `.cast` input ([asciicast
v2](https://docs.asciinema.org/manual/asciicast/v2/) or
[v3](https://docs.asciinema.org/manual/asciicast/v3/) — from `terminal-svg
rec`, any asciinema version, or anything else that writes the format)
renders as an animated SVG replaying the recording:

```sh
terminal-svg demo.cast -o demo.svg
```

asciinema 3 recordings embed the terminal's colours; render with them via
`-t auto` (see [themes.md](themes.md)).

**Re-render a terminal-svg SVG.** Every SVG this tool writes carries its
own source (see [round-tripping](#round-tripping-the-svg-is-its-own-source)),
so the SVG itself is an input — flags you pass override the options it was
rendered with, everything else stays put:

```sh
terminal-svg demo.svg -t nord --speed 2 -o demo-nord.svg
```

Output height always follows content (scrollback included), not the
terminal size — `-r` sets the PTY size programs see, not the image height.

## Recording: `terminal-svg rec`

`rec` records a live session and renders the animation when it ends:

```sh
# Record your shell; exit the shell to finish
terminal-svg rec -o demo.svg

# Record one command instead
terminal-svg rec -o build.svg -- cargo build
```

Alongside the SVG it saves the raw recording as an asciicast (same path with
a `.cast` extension, or wherever `--cast` points). That file is the master
copy: re-render with different flags without re-recording, or play it with
`asciinema play`.

```sh
terminal-svg demo.cast -t github-dark -o demo-dark.svg
terminal-svg demo.cast --speed 2 --no-loop -o demo-fast.svg
```

`rec` accepts every styling and animation flag below; its `-c`/`-r` default
to the current terminal's size rather than 80×24.

### How animations stay small

Rendered animations are aggressively compacted, so even minute-long sessions
stay in the tens of kilobytes: pauses are capped at 2 s
(`--idle-time-limit`), bursts of output are coalesced to ≤ 30 fps, identical
frames are deduplicated, and repeated rows are shared across frames via
`<defs>`/`<use>`. Playback loops with a 1.5 s hold on the last frame;
`--no-loop` plays once and freezes. The result is pure SVG/CSS — it animates
anywhere an `<img>` tag renders, GitHub READMEs included, no JavaScript.

## Round-tripping: the SVG is its own source

Rendered SVGs embed the original input — the `.cast` file byte-for-byte,
or the captured ANSI stream — plus the effective render options, deflate-
compressed inside a `<metadata>` block that renderers ignore. Two things
fall out of that:

```sh
# The SVG re-renders directly (see input modes above)
terminal-svg demo.svg -t github-dark -o dark.svg

# ...and the recording is recoverable, byte-exact
terminal-svg extract demo.svg -o demo.cast     # or to stdout without -o
```

Merged option precedence when re-rendering: command-line flags, then the
SVG's embedded options, then your config file, then built-in defaults.

Mind the two caveats:

- **The SVG contains the full capture** — everything programs wrote,
  which for `rec` sessions includes everything that echoed. Redact first
  (`terminal-svg edit --redact`), or disable embedding with
  `--no-embed-source` (flag or config key). Embedding typically adds a
  few percent to the file.
- **SVG optimizers strip `<metadata>`.** An SVG that went through svgo or
  similar loses its source; `extract` says so explicitly.

## Cleaning up recordings: `terminal-svg edit`

`edit` rewrites a `.cast` without re-recording. The output stays a
standard asciicast in the input's version (v2 → v2, v3 → v3, embedded v3
theme included); editing in place is refused so a bad pattern can't
destroy the original.

```sh
terminal-svg edit demo.cast \
  --redact 'ghp_[A-Za-z0-9]+' \     # mask matches with '*' (repeatable)
  --cut 12.5-20 \                   # remove 12.5s–20s, close the gap (repeatable)
  --max-pause 1.5 \                 # clamp pauses, baked into the file
  -o clean.cast                     # required; '-' writes to stdout
```

- `--redact` matches across event boundaries in both the output stream
  and the keystroke (input) stream, so a token split over several events
  — or typed character-by-character — is still caught. Masking is
  per-character, so layout and timing don't shift. Limitation: a secret
  interleaved with cursor-movement escape sequences can evade a regex;
  check the result before publishing.
- `--cut` ranges refer to the original timeline; multiple ranges are
  applied from the latest backwards so they don't interact.
- `--max-pause` is `--idle-time-limit` made permanent: the gaps are
  clamped in the file, and the header's own `idle_time_limit` is cleared
  so the cap isn't applied twice.

## The visual editor: `terminal-svg editor`

`editor` serves a live-preview UI for every render option and opens your
browser:

```sh
terminal-svg editor demo.cast            # tweak a recording visually
terminal-svg editor shot.ansi            # or an ANSI dump
terminal-svg editor demo.svg             # or re-edit any terminal-svg SVG
terminal-svg editor --port 7391 --no-open
```

The page is served by the binary itself on `127.0.0.1` (a random free
port unless `--port` says otherwise), and every preview is rendered by
the same pipeline the CLI uses — what you see is exactly what you get.
Drop a `.cast`, ANSI dump, or terminal-svg SVG onto the page to switch
files; **Download** saves through the browser and **Save** writes to the
`-o` path. Style and animation flags (`-t`, `--speed`, …) seed the
controls; a config file applies as usual. `ctrl-c` stops the server.

## Options

### Output and themes

| Flag | Default | |
|---|---|---|
| `-o, --output <PATH>` | `terminal.svg` | `-` writes the SVG to stdout |
| `-t, --theme <THEME>` | `dracula` | built-in name, path to a `.toml`, or `auto` for the palette embedded in an asciicast v3 — see [themes.md](themes.md) |
| `--theme-light <THEME>` | | with `--theme-dark`: emit both palettes in one SVG, switched by the viewer's `prefers-color-scheme`; works for static and animated output |
| `--theme-dark <THEME>` | | the dark half of the pair |
| `--no-embed-source` | | don't embed the source recording in the SVG (disables [round-tripping](#round-tripping-the-svg-is-its-own-source)) |
| `--list-themes` | | print built-in theme names and exit |

### Window

| Flag | Default | |
|---|---|---|
| `--chrome <STYLE>` | `macos` | `macos`, `windows`, `ubuntu`, or `none`; chrome is fixed-size like a real window and doesn't scale with `--font-size` |
| `--title <TITLE>` | auto | see [title detection](#title-detection) |
| `--title-emoji <EMOJI>` | 📁 for paths | emoji before the title; `""` disables |
| `--no-window` | | bare rounded panel (alias for `--chrome none`) |
| `--no-background` | | fully transparent: no window body, chrome, shadow, or margin |
| `--no-shadow` | | keep the window, drop the shadow (margin becomes 0) |

### Layout and fonts

| Flag | Default | |
|---|---|---|
| `--font-size <PX>` | `14` | |
| `--line-height <N>` | `1.2` | multiple of font size |
| `--padding <PX>` | `10` | between window edge and text |
| `--margin <PX>` | `24` | around the window; defaults to 0 when there's no shadow |
| `--no-font-embed` | | reference system fonts instead of embedding the subset — smaller file, but alignment then depends on the viewer's fonts |
| `--font-family <NAME>` | JetBrains Mono stack | family to reference with `--no-font-embed` |

### Capture

| Flag | Default | |
|---|---|---|
| `-c, --cols <N>` | `80` (`rec`: current terminal) | PTY width programs see |
| `-r, --rows <N>` | `24` (`rec`: current terminal) | PTY height; image height still follows content |
| `--timeout <SECS>` | | kill the PTY command after N seconds and render what was captured — handy for `tail -f`-ish commands and CI |
| `--cast <PATH>` | output stem + `.cast` | (`rec` only) where the asciicast is saved |

### Animation

These apply when the input is a `.cast` file or a `rec` session:

| Flag | Default | |
|---|---|---|
| `--idle-time-limit <SECS>` | recording's own, or `2` | cap pauses between events |
| `--speed <N>` | `1` | playback speed multiplier |
| `--no-loop` | | play once and hold the last frame |
| `--from <SECS>` | | start the animation here; the first frame shows the screen as of this moment |
| `--to <SECS>` | | end the animation here |
| `--cursor <STYLE>` | `block` | cursor shape: `block`, `bar`, `underline`, or `none` |
| `--static` | | render only the final screen, no animation |
| `--at <SECS>` | | render the screen at this point in the recording (implies `--static`) |

Animated SVGs respect the viewer's reduced-motion preference: with
`prefers-reduced-motion: reduce` the animation is disabled and the final
frame shows as a still poster.

## Configuration file

Personal defaults live in `~/.config/terminal-svg/config.toml`
(`$XDG_CONFIG_HOME` is respected; `--config <path>` points somewhere
else). Keys mirror the long flag names; anything set on the command line
still wins:

```toml
theme = "nord"
chrome = "none"
cursor = "bar"
font-size = 16.0
line-height = 1.3
padding = 12.0
# margin = 32.0
# title-emoji = "🚀"
# font-family = "Menlo,monospace"     # used with --no-font-embed
# theme-light = "github-light"       # set both or neither
# theme-dark = "github-dark"
# speed = 1.5
# idle-time-limit = 3.0
# no-shadow = true
# no-embed-source = true
```

Unknown keys are an error (typos fail loudly rather than silently doing
nothing). Without a config file, nothing changes — there is no file until
you create one.

## Title detection

The title bar text is picked in order:

1. `--title`, if given;
2. the recording's own title (asciicast header), for `.cast` input;
3. the last title the program set via OSC 0/2 — shells that report their
   working directory show up Ghostty-style as `📁 ~/Code/blog`;
4. the command string, for PTY captures.

When the detected title looks like a path it gets a 📁 prefix;
`--title-emoji` swaps the emoji or (with `""`) removes it.

## Recipes

A README image that follows the viewer's light/dark mode — animated
recordings share one set of frames between the two palettes, so the dual
document costs barely anything over a single theme (add `--static` for a
still):

```sh
terminal-svg demo.cast --theme-light github-light --theme-dark github-dark
```

Faithful Windows PowerShell and Ubuntu GNOME Terminal windows (chrome and
theme are independent — these pairings are just the authentic ones):

```sh
terminal-svg --chrome windows -t powershell -- pwsh -c 'Get-ChildItem'
terminal-svg --chrome ubuntu -t ubuntu -- lsd -la
```

A transparent panel for slides or compositing over your own background:

```sh
terminal-svg --no-background -- lsd -la
```

Screenshot a long-running command in CI:

```sh
terminal-svg --timeout 10 -o logs.svg -- kubectl logs -f my-pod
```

Freeze-frame a moment out of a recording:

```sh
terminal-svg demo.cast --at 3.5 -o midpoint.svg
```

Animate just a slice of a recording — the first frame opens on the screen
as of `--from`, so the lead-in isn't replayed:

```sh
terminal-svg demo.cast --from 12 --to 31 -o highlight.svg
```

Pipe the SVG onward instead of writing a file:

```sh
terminal-svg -o - -- lsd -la | svgo -i - -o shot.min.svg
```

## Viewing the output

Browsers render the embedded WOFF2 correctly. macOS Quick Look does not —
it silently ignores embedded fonts, so glyphs fall back and columns drift.
Judge output in a browser, or headless Chrome for scripted checks:

```sh
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --headless --screenshot=check.png --window-size=900x600 file://$PWD/terminal.svg
```
