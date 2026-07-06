#!/usr/bin/env python3
"""Generate docs/assets/logo.cast — the animated logo wordmark recording.

The logo is terminal-svg output, made with terminal-svg: a green prompt
chevron types "terminal-svg" char by char, then the block cursor blinks
(real DECTCEM hide/show events) before the loop restarts. Fully
deterministic. Regenerate the logo with:

    python3 scripts/make-logo-cast.py
    cargo run --release -- docs/assets/logo.cast -o docs/assets/logo.svg \
        --no-background --font-size 28 --idle-time-limit 3 \
        --theme-light github-light --theme-dark github-dark
"""
import json
import pathlib

ESC = chr(27)
BGREEN = f"{ESC}[1;32m"
RESET = f"{ESC}[0m"
HIDE = f"{ESC}[?25l"
SHOW = f"{ESC}[?25h"

events = [
    (0.0, f"{BGREEN}❯{RESET} "),
    # type the name with human-ish cadence
    (0.9, "t"), (1.03, "e"), (1.12, "r"), (1.26, "m"), (1.38, "i"),
    (1.46, "n"), (1.59, "a"), (1.73, "l"), (1.98, "-"), (2.11, "s"),
    (2.22, "v"), (2.34, "g"),
    # let the cursor blink twice before the loop comes around
    (3.0, HIDE), (3.6, SHOW), (4.2, HIDE), (4.8, SHOW),
    (6.3, ""),
]

header = {"version": 2, "width": 15, "height": 1, "title": "terminal-svg"}
lines = [json.dumps(header)]
lines += [json.dumps([t, "o", data]) for t, data in events]

out = pathlib.Path(__file__).resolve().parent.parent / "docs/assets/logo.cast"
out.write_text("\n".join(lines) + "\n")
print(f"wrote {out}")
