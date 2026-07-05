#!/usr/bin/env python3
"""Generate docs/demo.cast — the README demo recording for terminal-svg.

Hand-authored timeline showing off: char-by-char typing, a braille spinner,
a carriage-return progress bar, ANSI colours/bold/underline, box drawing,
and an emoji fallback run. Fully deterministic. Regenerate the README image
with:

    python3 examples/make-demo-cast.py
    cargo run --release -- docs/demo.cast -o docs/demo.svg
"""
import json
import pathlib

events = []
t = 0.0


def out(delay, data):
    global t
    t = round(t + delay, 6)
    events.append([t, "o", data])


ESC = ""
GREEN = f"{ESC}[32m"
BGREEN = f"{ESC}[1;32m"
CYAN = f"{ESC}[36m"
GRAY = f"{ESC}[90m"
YELLOW = f"{ESC}[33m"
MAGENTA = f"{ESC}[35m"
BOLD = f"{ESC}[1m"
ULCYAN = f"{ESC}[4;36m"
R = f"{ESC}[0m"

PROMPT = f"{BGREEN}➜{R} {CYAN}~/app{R} "

# Prompt appears, then the command is typed char by char.
out(0.12, PROMPT)
delays = [0.14, 0.09, 0.07, 0.11, 0.06, 0.08, 0.12, 0.07, 0.06, 0.09,
          0.21, 0.08, 0.06, 0.10, 0.07, 0.09, 0.06, 0.08]
for ch, d in zip("./deploy.sh --prod", delays):
    out(d, ch)
out(0.35, "\r\n")

out(0.25, f"{GRAY}deploy v2.1.0 — production{R}\r\n")

# Spinner while "building assets", resolves to a green check.
SPIN = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
out(0.20, f"{SPIN[0]} building assets")
for i in range(1, 14):
    out(0.08, f"\r{SPIN[i % len(SPIN)]} building assets")
out(0.15, f"\r{GREEN}✓{R} built 38 modules in 1.2s   \r\n")

# Progress bar drawn with carriage-return overwrites.
for pct in range(0, 101, 5):
    filled = "█" * (pct * 20 // 100)
    empty = "░" * (20 - pct * 20 // 100)
    out(0.11, f"\r↑ uploading  {CYAN}{filled}{empty}{R} {pct:3}%")
out(0.18, f"\r{GREEN}✓{R} uploaded 42 files (3.1 MB)          \r\n")
out(0.30, f"{YELLOW}! cache cold — warming 12 edge nodes{R}\r\n")

# Result panel: box drawing + emoji + underlined URL.
out(0.55, f"\r\n{MAGENTA}╭{'─' * 34}╮{R}\r\n")
out(0.06, f"{MAGENTA}│{R}  {BOLD}deployed to production{R} \U0001f680       {MAGENTA}│{R}\r\n")
out(0.06, f"{MAGENTA}│{R}  {ULCYAN}https://app.example.com{R}         {MAGENTA}│{R}\r\n")
out(0.06, f"{MAGENTA}╰{'─' * 34}╯{R}\r\n")

# Back at the prompt; the cursor rests here through the trailing pause.
out(0.60, f"\r\n{PROMPT}")

header = {"version": 2, "width": 58, "height": 13, "title": "terminal-svg rec"}

dest = pathlib.Path(__file__).resolve().parent.parent / "docs" / "demo.cast"
with open(dest, "w") as f:
    f.write(json.dumps(header) + "\n")
    for time, code, data in events:
        f.write(json.dumps([time, code, data], ensure_ascii=False) + "\n")

print(f"events: {len(events)}, duration: {t:.2f}s")
