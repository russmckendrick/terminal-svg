# Installing terminal-svg

terminal-svg is a single self-contained binary — the font is baked in, there
are no runtime dependencies, and nothing else to install. Pick whichever
route below suits.

## Homebrew (macOS and Linux)

```sh
brew install russmckendrick/tap/terminal-svg
```

The [tap formula](https://github.com/russmckendrick/homebrew-tap) covers
macOS and Linux, on both Apple Silicon/ARM and Intel/AMD, and installs the
same binaries as the release page. Upgrade with `brew upgrade terminal-svg`.

## Prebuilt binaries from GitHub releases

Every tagged release on the
[releases page](https://github.com/russmckendrick/terminal-svg/releases)
ships a binary per platform, each with a matching `.sha256` checksum file:

| Platform | File |
|---|---|
| Linux x86_64 | `terminal-svg-linux-amd64` |
| Linux ARM64 | `terminal-svg-linux-arm64` |
| macOS Apple Silicon | `terminal-svg-darwin-arm64` |
| macOS Intel | `terminal-svg-darwin-amd64` |
| Windows x86_64 | `terminal-svg-windows-amd64.exe` |

The names are stable across releases, so
`https://github.com/russmckendrick/terminal-svg/releases/latest/download/<file>`
always fetches the newest version.

### Linux

```sh
curl -LO https://github.com/russmckendrick/terminal-svg/releases/latest/download/terminal-svg-linux-amd64
chmod +x terminal-svg-linux-amd64
sudo mv terminal-svg-linux-amd64 /usr/local/bin/terminal-svg
```

On ARM (Raspberry Pi, AWS Graviton, etc.) swap `amd64` for `arm64`. No
`sudo`? Put it somewhere on your own `PATH` instead, e.g.
`~/.local/bin/terminal-svg`.

The Linux binaries are glibc builds (`*-unknown-linux-gnu`) and run on any
mainstream distro from the last several years; on musl-based systems like
Alpine, [build from source](#building-from-source).

### macOS

Homebrew above is the easy path. To install the raw binary instead:

```sh
curl -LO https://github.com/russmckendrick/terminal-svg/releases/latest/download/terminal-svg-darwin-arm64
chmod +x terminal-svg-darwin-arm64
sudo mv terminal-svg-darwin-arm64 /usr/local/bin/terminal-svg
```

Use `darwin-amd64` on Intel Macs. The binaries are not notarized: fetched
with `curl` they run as-is, but if you download one through a browser,
Gatekeeper will quarantine it — clear that with:

```sh
xattr -d com.apple.quarantine terminal-svg
```

### Windows

In PowerShell:

```powershell
# Somewhere on your PATH — adjust to taste
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\terminal-svg" | Out-Null
Invoke-WebRequest `
  -Uri https://github.com/russmckendrick/terminal-svg/releases/latest/download/terminal-svg-windows-amd64.exe `
  -OutFile "$env:LOCALAPPDATA\Programs\terminal-svg\terminal-svg.exe"
```

Then add that folder to your `PATH` if it isn't already (takes effect in new
terminals):

```powershell
[Environment]::SetEnvironmentVariable(
  "Path",
  [Environment]::GetEnvironmentVariable("Path", "User") + ";$env:LOCALAPPDATA\Programs\terminal-svg",
  "User")
```

The executable is unsigned, so SmartScreen may warn on first run —
"More info → Run anyway", or verify the checksum first (below) if you'd
rather not take a stranger's word for it.

### Verifying checksums

Each binary has a `<file>.sha256` next to it on the release page, containing
the SHA-256 in the standard `<hash>  <filename>` format.

```sh
# Linux
curl -LO https://github.com/russmckendrick/terminal-svg/releases/latest/download/terminal-svg-linux-amd64.sha256
sha256sum -c terminal-svg-linux-amd64.sha256

# macOS
curl -LO https://github.com/russmckendrick/terminal-svg/releases/latest/download/terminal-svg-darwin-arm64.sha256
shasum -a 256 -c terminal-svg-darwin-arm64.sha256
```

```powershell
# Windows — compare the two hashes by eye
(Get-FileHash terminal-svg-windows-amd64.exe -Algorithm SHA256).Hash.ToLower()
Get-Content terminal-svg-windows-amd64.exe.sha256
```

### Checking it works

```sh
terminal-svg --version
terminal-svg --list-themes
echo hello | terminal-svg -o hello.svg
```

## Building from source

You need a Rust toolchain (1.85 or newer — the crate uses the 2024 edition);
[rustup](https://rustup.rs) is the usual way to get one. The JetBrainsMono
Nerd Font faces in `assets/fonts/` are vendored in the repo and baked into
the binary at build time, so there's nothing extra to fetch.

```sh
# Straight from GitHub
cargo install --git https://github.com/russmckendrick/terminal-svg

# Or from a checkout
git clone https://github.com/russmckendrick/terminal-svg
cd terminal-svg
cargo install --path .
```

Both put `terminal-svg` in `~/.cargo/bin`. For a binary without installing,
`cargo build --release` leaves it at `target/release/terminal-svg`.

## Uninstalling

- Homebrew: `brew uninstall terminal-svg`
- Manual binary: delete it (`/usr/local/bin/terminal-svg` or wherever you
  put it)
- Cargo: `cargo uninstall terminal-svg`

terminal-svg writes nothing outside the SVGs (and `.cast` files) you ask it
for — no config directories or caches to clean up.
