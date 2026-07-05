use std::collections::BTreeSet;

use allsorts::binary::read::ReadScope;
use allsorts::font_data::FontData;
use allsorts::subset::{CmapTarget, SubsetProfile, subset};
use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

/// Subset a face down to `chars` and return it as base64 WOFF2, ready for a
/// data: URI. Returns None when no requested char is covered by the face
/// (embedding it would be pure dead weight). Uncovered chars are silently
/// skipped — they render via the fallback font stack.
pub fn woff2_base64(font_data: &[u8], chars: &BTreeSet<char>) -> Result<Option<String>> {
    let face = ttf_parser::Face::parse(font_data, 0).context("failed to parse bundled font")?;

    let mut glyph_ids: BTreeSet<u16> = BTreeSet::new();
    glyph_ids.insert(0); // .notdef is mandatory
    for &c in chars {
        if let Some(glyph) = face.glyph_index(c) {
            glyph_ids.insert(glyph.0);
        }
    }
    if glyph_ids.len() <= 1 {
        return Ok(None);
    }

    let scope = ReadScope::new(font_data);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|e| anyhow!("failed to read font tables: {e}"))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|e| anyhow!("failed to open font table provider: {e}"))?;

    let ids: Vec<u16> = glyph_ids.into_iter().collect();
    // Unicode cmap: browsers reject Mac Roman-only cmaps.
    let subset_ttf = subset(
        &provider,
        &ids,
        &SubsetProfile::Minimal,
        CmapTarget::Unicode,
    )
    .map_err(|e| anyhow!("font subsetting failed: {e}"))?;

    let woff2 = ttf2woff2::encode(&subset_ttf, ttf2woff2::BrotliQuality::default())
        .map_err(|e| anyhow!("woff2 encoding failed: {e}"))?;
    Ok(Some(BASE64.encode(woff2)))
}

/// Predicate for "the bundled face has a glyph for this char", used to split
/// runs so uncovered chars (emoji) get their own explicitly-positioned run
/// and fallback-font advance drift cannot shift later columns.
pub fn coverage(font_data: &[u8]) -> Result<impl Fn(char) -> bool + '_> {
    let face = ttf_parser::Face::parse(font_data, 0).context("failed to parse bundled font")?;
    Ok(move |c: char| face.glyph_index(c).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::assets;

    #[test]
    fn subsets_to_used_glyphs_only() {
        let chars: BTreeSet<char> = "hello ─│┌".chars().collect();
        let b64 = woff2_base64(assets::regular(), &chars).unwrap().unwrap();
        let woff2 = BASE64.decode(&b64).unwrap();
        // WOFF2 magic 'wOF2'
        assert_eq!(&woff2[..4], b"wOF2");
        // A handful of glyphs must be tiny compared to the 2.4 MB source.
        assert!(woff2.len() < 20_000, "subset is {} bytes", woff2.len());
    }

    #[test]
    fn returns_none_when_nothing_covered() {
        let chars: BTreeSet<char> = "😀🎉".chars().collect();
        assert!(woff2_base64(assets::regular(), &chars).unwrap().is_none());
    }

    #[test]
    fn nerd_font_covers_powerline() {
        let covered = coverage(assets::regular()).unwrap();
        assert!(covered('\u{e0b0}')); // powerline right arrow
        assert!(covered('─'));
        assert!(!covered('😀'));
    }
}
