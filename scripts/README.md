# Scripts

Development helpers — nothing here is needed to build or use terminal-svg.
All three are run from the repo root and write their output into it.

## gallery.sh

```sh
./scripts/gallery.sh && open gallery.html
```

The visual sweep: builds a debug binary, renders every fixture in
`tests/fixtures/` (the `.ansi` ones statically, the `.cast` ones animated)
in every built-in theme, and writes the SVGs to `gallery/` with a
`gallery.html` index for eyeballing them side by side. OS-flavoured themes
render in their native chrome (`powershell` → `--chrome windows`,
`ubuntu` → `--chrome ubuntu`), so those views get checked too.

The `gallery/` SVGs are committed: the website's gallery page is built from
them (`site/src/lib/gallery.ts` globs `gallery/*.svg`), and CI can't run
interactive fixtures. After an intentional rendering change, rerun the
script and commit the diff alongside the golden updates.

## make-demo-cast.py

```sh
python3 scripts/make-demo-cast.py
cargo run --release -- docs/assets/demo.cast -o docs/assets/demo.svg
```

Generates `docs/assets/demo.cast`, the recording behind the README's
animated demo. It's a hand-authored, fully deterministic timeline (no
actual recording session) showing off char-by-char typing, a braille
spinner, a carriage-return progress bar, colours, box drawing, and emoji
fallback — edit the script and rerun rather than re-recording. Plain
Python, no dependencies.

## make-fixtures.sh

```sh
./scripts/make-fixtures.sh
```

Regenerates `tests/fixtures/*.ansi`, the inputs for the golden tests and
the gallery. The fixtures are checked in and **frozen** — rerun this only
when adding a new fixture, then regenerate the goldens
(`UPDATE_GOLDEN=1 cargo test --test golden`) and review the diff.
