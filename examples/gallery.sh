#!/usr/bin/env bash
# Renders every fixture in every built-in theme into gallery.html for a
# quick visual sweep: ./examples/gallery.sh && open gallery.html
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

for theme in $("$BIN" --list-themes); do
  echo "<h2>$theme</h2>" >> "$HTML"
  for fixture in tests/fixtures/*.ansi; do
    name=$(basename "$fixture" .ansi)
    svg="$OUT/$theme-$name.svg"
    "$BIN" "$fixture" -t "$theme" --title "$name" -c 70 -o "$svg" 2>/dev/null
    echo "<img src=\"$svg\" alt=\"$theme $name\">" >> "$HTML"
  done
done

echo "wrote $HTML ($(ls "$OUT" | wc -l | tr -d ' ') SVGs)"
