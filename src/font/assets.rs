use std::io::Read;
use std::sync::OnceLock;

static REGULAR: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/JetBrainsMonoNerdFontMono-Regular.ttf.deflate"
));
static BOLD: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/JetBrainsMonoNerdFontMono-Bold.ttf.deflate"
));

fn decompress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    flate2::read::DeflateDecoder::new(data)
        .read_to_end(&mut out)
        .expect("bundled font decompression cannot fail");
    out
}

pub fn regular() -> &'static [u8] {
    static CELL: OnceLock<Vec<u8>> = OnceLock::new();
    CELL.get_or_init(|| decompress(REGULAR))
}

pub fn bold() -> &'static [u8] {
    static CELL: OnceLock<Vec<u8>> = OnceLock::new();
    CELL.get_or_init(|| decompress(BOLD))
}
