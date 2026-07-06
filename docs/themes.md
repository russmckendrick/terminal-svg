# Theme format

A theme is a single TOML file. Pass it with `-t`:

```sh
terminal-svg -t path/to/my-theme.toml -- htop
```

Anything with a `/` in it or ending in `.toml` is treated as a file path;
otherwise the name is looked up in the built-ins (`terminal-svg --list-themes`).

## Full reference

```toml
name = "my-theme"

[colors]                      # all 18 keys required, #rrggbb or #rgb
foreground = "#f8f8f2"        # default text colour
background = "#282a36"        # window body colour

black = "#21222c"             # ANSI 0-7
red = "#ff5555"
green = "#50fa7b"
yellow = "#f1fa8c"
blue = "#bd93f9"
magenta = "#ff79c6"
cyan = "#8be9fd"
white = "#f8f8f2"

bright_black = "#6272a4"      # ANSI 8-15
bright_red = "#ff6e6e"
bright_green = "#69ff94"
bright_yellow = "#ffffa5"
bright_blue = "#d6acff"
bright_magenta = "#ff92df"
bright_cyan = "#a4ffff"
bright_white = "#ffffff"

[chrome]                      # everything here is optional
title_fg = "#6272a4"          # default: foreground blended 45% toward background
shadow_opacity = 0.35         # default: 0.35 (github-light uses 0.25)
light_close = "#ff5f57"       # traffic light overrides; defaults are the
light_minimize = "#febc2e"    #   standard macOS red/amber/green
light_zoom = "#28c840"
button_fg = "#6272a4"         # caption glyphs (--chrome windows/ubuntu);
                              #   default: title_fg
button_bg = "#2f3240"         # button discs (--chrome ubuntu); default:
                              #   title_fg blended 85% toward background
bar_bg = "#ffffff"            # title bar fill (--chrome windows/ubuntu);
                              #   default: the authentic OS chrome color
bar_fg = "#000000"            # title/glyph color on that bar
```

## What themes do and don't control

- **ANSI 0–15** come from the palette above.
- **ANSI 16–231** (the 6×6×6 colour cube) and **232–255** (grayscale ramp)
  are computed from the xterm standard formula and are the same in every
  theme — this is what real terminals do.
- **Truecolor** (`ESC[38;2;r;g;b m`) passes straight through untouched.
- **Faint** text is the foreground blended 50% toward the effective
  background; **inverse** swaps foreground and background. Bold does *not*
  auto-brighten colours 0–7 (matching modern terminal defaults).

## Contributing a built-in

Add the TOML to [themes/](../themes/), register it in
`src/theme/builtin.rs`, and check the `name` field matches the registry key
(a test enforces this). Source palette values from the scheme's official
palette documentation, not from screenshots.
