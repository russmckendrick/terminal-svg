#!/usr/bin/env bash
# Renders every fixture in every built-in theme into gallery.html for a
# quick visual sweep: ./scripts/gallery.sh && open gallery.html
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --quiet
BIN=target/debug/terminal-svg
OUT=gallery
mkdir -p "$OUT"

HTML=gallery.html
cat > "$HTML" <<'EOF'
<!doctype html>
<meta charset="utf-8">
<title>terminal-svg gallery</title>
<style>
  body { background: #3c3f4a; font-family: system-ui; padding: 2rem; }
  h2 { color: #eee; font-weight: 600; margin: 2rem 0 0.5rem; }
  img { display: inline-block; vertical-align: top; margin: 0 1rem 1rem 0; max-width: 46%; }
</style>
EOF

# The OS-flavoured themes render in their native window chrome, so the
# gallery shows the Windows and Ubuntu views alongside the macOS ones.
chrome_for() {
  case "$1" in
    powershell) echo windows ;;
    ubuntu) echo ubuntu ;;
    *) echo macos ;;
  esac
}

for theme in $("$BIN" --list-themes); do
  chrome=$(chrome_for "$theme")
  echo "<h2>$theme</h2>" >> "$HTML"
  for fixture in tests/fixtures/*.ansi; do
    name=$(basename "$fixture" .ansi)
    svg="$OUT/$theme-$name.svg"
    "$BIN" "$fixture" -t "$theme" --chrome "$chrome" --title "$name" -c 70 -o "$svg" 2>/dev/null
    echo "<img src=\"$svg\" alt=\"$theme $name\">" >> "$HTML"
  done
  # Animated: replay the checked-in cast fixture (deterministic, no
  # interactive step).
  for fixture in tests/fixtures/*.cast; do
    name=$(basename "$fixture" .cast)
    svg="$OUT/$theme-$name-anim.svg"
    "$BIN" "$fixture" -t "$theme" --chrome "$chrome" --title "$name" -o "$svg" 2>/dev/null
    echo "<img src=\"$svg\" alt=\"$theme $name animated\">" >> "$HTML"
  done
done

echo "wrote $HTML ($(ls "$OUT" | wc -l | tr -d ' ') SVGs)"
