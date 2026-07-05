#!/usr/bin/env bash
# Regenerates tests/fixtures/*.ansi. Fixtures are checked in and frozen;
# rerun this only when adding a new fixture.
set -euo pipefail
cd "$(dirname "$0")/../tests/fixtures"

printf 'normal \e[30mblack\e[31mred\e[32mgreen\e[33myellow\e[34mblue\e[35mmagenta\e[36mcyan\e[37mwhite\e[0m\n\e[90mBblack\e[91mBred\e[92mBgreen\e[93mByellow\e[94mBblue\e[95mBmagenta\e[96mBcyan\e[97mBwhite\e[0m\n\e[41m red bg \e[44m blue bg \e[102m bright green bg \e[0m\n' > colors16.ansi

{
  for i in 16 21 46 82 118 154 190 196 202 208 214 220 226 232 240 248 255; do
    printf '\e[38;5;%dm%03d\e[0m ' "$i" "$i"
  done
  printf '\n'
  for i in 17 22 52 88 124 160 233 245 254; do
    printf '\e[48;5;%dm %03d \e[0m' "$i" "$i"
  done
  printf '\n'
} > colors256.ansi

printf '\e[38;2;255;105;180mhot pink fg\e[0m \e[48;2;25;100;200;38;2;255;255;0myellow on azure\e[0m\n\e[38;2;80;250;123mdracula green truecolor\e[0m\n' > truecolor.ansi

printf 'plain \e[1mbold\e[0m \e[2mfaint\e[0m \e[3mitalic\e[0m \e[4munderline\e[0m \e[9mstrike\e[0m \e[7minverse\e[0m\n\e[1;3mbold-italic\e[0m \e[4;9munder-strike\e[0m \e[1;4;31mbold-under-red\e[0m \e[5mblink\e[0m\n\e[7;32mgreen-inverse\e[0m \e[2;34mfaint-blue\e[0m \e[4;44munderline-on-bg\e[0m\n' > sgr-styles.ansi

# Literal UTF-8 so the geometry is reviewable: every row is 16 columns.
cat > boxdrawing.ansi <<'EOF'
┏━━ heavy ━━━━━┓
┃ ╭─ light ─╮  ┃
┃ ╰─────────╯  ┃
┗━━━━━━━━━━━━━━┛
░▒▓█ blocks █▓▒░
EOF

# \r overwrites, ESC[K clear-to-eol, cursor-up repaint: only the final
# frame must survive.
printf 'downloading:  10%%\rdownloading:  55%%\rdownloading: 100%%\ninstalling: [....      ]\r\e[Kinstalling: [##########] ok\nstep 1 pending\nstep 2 pending\e[1A\rstep 1 \e[32mdone\e[0m   \e[1B\rstep 2 \e[32mdone\e[0m   \n' > progress.ansi

printf 'cjk: \xe6\xbc\xa2\xe5\xad\x97\xe3\x81\x8b\xe3\x81\xaa\xed\x95\x9c\xea\xb5\xad wide\nemoji: \xf0\x9f\x9a\x80 \xf0\x9f\x8e\x89 mixed \xe6\x97\xa5\xe6\x9c\xac\xf0\x9f\x97\xbe end\ncombining: cafe\xcc\x81 done\n' > cjk-emoji.ansi

# starship-style two-segment powerline prompt
printf '\e[48;5;24;97m \xee\x82\xa0 main \e[0m\e[38;5;24;48;5;238m\xee\x82\xb0\e[0m\e[48;5;238;93m \xf0\x9f\xa6\x80 v1.95.0 \e[0m\e[38;5;238m\xee\x82\xb0\e[0m \e[1;32m\xe2\x9d\xaf\e[0m cargo build\n\e[2m   Compiling\e[0m terminal-svg v0.1.0\n\e[1;32m    Finished\e[0m dev profile in 7.5s\n' > starship.ansi

echo "fixtures written:"
ls -la
