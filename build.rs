use std::io::Write;
use std::path::Path;
use std::{env, fs};

const FONTS: &[&str] = &[
    "JetBrainsMonoNerdFontMono-Regular.ttf",
    "JetBrainsMonoNerdFontMono-Bold.ttf",
];

fn main() {
    println!("cargo::rerun-if-changed=assets/fonts");
    let out_dir = env::var("OUT_DIR").unwrap();
    for name in FONTS {
        let bytes = fs::read(Path::new("assets/fonts").join(name))
            .unwrap_or_else(|e| panic!("missing font asset {name}: {e}"));
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&bytes).unwrap();
        let compressed = encoder.finish().unwrap();
        fs::write(
            Path::new(&out_dir).join(format!("{name}.deflate")),
            compressed,
        )
        .unwrap();
    }
}
