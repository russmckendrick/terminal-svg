//! Source embedding: the SVG carries its own source.
//!
//! Every rendered SVG gets a `<metadata>` block holding the original
//! input (the .cast file bytes, or the captured ANSI stream) plus the
//! effective `RenderOptions`, deflate-compressed and base64-encoded. That
//! makes the SVG re-renderable (`terminal-svg shot.svg -t nord`) and the
//! recording recoverable (`terminal-svg extract shot.svg`) with no .cast
//! file to keep track of. Browsers ignore `<metadata>`, so rendering is
//! unaffected; SVG optimizers like svgo strip it, which forfeits the
//! round-trip.
//!
//! The payload is deterministic — raw deflate (no gzip mtime header) and
//! stable JSON field order — so re-rendering identical input with
//! identical options produces a byte-identical SVG.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::options::RenderOptions;

/// Marker attribute on the metadata element; also what
/// `looks_like_terminal_svg` sniffs for.
const MARKER: &str = "terminal-svg-source";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// An asciicast recording: `data` is the .cast file, byte-exact.
    Cast,
    /// A raw ANSI byte stream (PTY capture, pipe, or .ansi dump).
    Ansi,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            SourceKind::Cast => "cast",
            SourceKind::Ansi => "ansi",
        }
    }

    /// The natural file extension for an extracted source.
    pub fn extension(self) -> &'static str {
        match self {
            SourceKind::Cast => "cast",
            SourceKind::Ansi => "ansi",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddedSource {
    pub kind: SourceKind,
    pub data: Vec<u8>,
    pub options: RenderOptions,
}

/// Insert the source metadata block right after the opening `<svg …>` tag.
pub fn embed(svg: &str, source: &EmbeddedSource) -> String {
    let insert_at = svg
        .find("<svg")
        .and_then(|start| svg[start..].find('>').map(|end| start + end + 1))
        .unwrap_or(0);

    let options_json = serde_json::to_string(&source.options).expect("options serialize");
    let mut deflated = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    deflated.write_all(&source.data).expect("deflate to vec");
    let data_b64 = BASE64.encode(deflated.finish().expect("deflate to vec"));

    let block = format!(
        r#"<metadata id="{MARKER}"><tsvg:source xmlns:tsvg="https://terminal-svg.dev/ns/source/1" version="1" kind="{kind}"><tsvg:options>{options}</tsvg:options><tsvg:data encoding="deflate+base64">{data}</tsvg:data></tsvg:source></metadata>"#,
        kind = source.kind.as_str(),
        options = xml_text_escape(&options_json),
        data = data_b64,
    );

    let mut out = String::with_capacity(svg.len() + block.len() + 1);
    out.push_str(&svg[..insert_at]);
    out.push('\n');
    out.push_str(&block);
    out.push_str(&svg[insert_at..]);
    out
}

/// Recover the embedded source from an SVG. `Ok(None)` means the document
/// has no terminal-svg source block (a foreign SVG, or one that went
/// through an optimizer); malformed blocks are errors.
pub fn extract(svg: &str) -> Result<Option<EmbeddedSource>> {
    let Some(meta_start) = svg.find(&format!(r#"<metadata id="{MARKER}">"#)) else {
        return Ok(None);
    };
    let block = svg[meta_start..]
        .split_once("</metadata>")
        .map(|(b, _)| b)
        .ok_or_else(|| anyhow!("unterminated {MARKER} metadata block"))?;

    let kind = match attr_value(block, "kind") {
        Some("cast") => SourceKind::Cast,
        Some("ansi") => SourceKind::Ansi,
        Some(other) => bail!("unknown embedded source kind {other:?}"),
        None => bail!("embedded source block has no kind attribute"),
    };

    let options_text = element_text(block, "tsvg:options")
        .ok_or_else(|| anyhow!("embedded source block has no tsvg:options"))?;
    let options: RenderOptions = serde_json::from_str(&xml_text_unescape(options_text))
        .context("embedded render options do not parse")?;

    let data_b64 = element_text(block, "tsvg:data")
        .ok_or_else(|| anyhow!("embedded source block has no tsvg:data"))?;
    let deflated = BASE64
        .decode(data_b64.trim())
        .context("embedded source data is not valid base64")?;
    let mut data = Vec::new();
    flate2::read::DeflateDecoder::new(&deflated[..])
        .read_to_end(&mut data)
        .context("embedded source data does not inflate")?;

    Ok(Some(EmbeddedSource {
        kind,
        data,
        options,
    }))
}

/// Cheap sniff for the input dispatcher: an .svg file whose first few KB
/// carry the source marker.
pub fn looks_like_terminal_svg(path: &Path) -> bool {
    if path.extension().is_none_or(|e| e != "svg") {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 4096];
    let mut filled = 0;
    while filled < head.len() {
        match f.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return false,
        }
    }
    String::from_utf8_lossy(&head[..filled]).contains(MARKER)
}

/// The text content of the first `<tag …>…</tag>` element in `block`.
fn element_text<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = block.find(&format!("<{tag}"))?;
    let text_start = open + block[open..].find('>')? + 1;
    let text_end = text_start + block[text_start..].find(&format!("</{tag}>"))?;
    Some(&block[text_start..text_end])
}

fn attr_value<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let at = block.find(&format!(r#"{name}=""#))? + name.len() + 2;
    block[at..].split_once('"').map(|(v, _)| v)
}

/// Escape a JSON document for use as an XML text node. Only `&` and `<`
/// need encoding; `>` is left alone for readability.
fn xml_text_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;")
}

fn xml_text_unescape(text: &str) -> String {
    // "&lt;" cannot occur inside an escaped "&amp;…" run, so this order
    // is safe (escape encoded any original "&lt;" as "&amp;lt;").
    text.replace("&lt;", "<").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EmbeddedSource {
        EmbeddedSource {
            kind: SourceKind::Cast,
            data: br#"{"version": 2, "width": 80, "height": 24}
[0.1, "o", "hi"]
"#
            .to_vec(),
            options: RenderOptions {
                theme: Some("nord".into()),
                speed: Some(2.0),
                no_shadow: true,
                ..Default::default()
            },
        }
    }

    #[test]
    fn embed_extract_round_trips_byte_exact() {
        let svg =
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\">\n<rect/>\n</svg>";
        let source = sample();
        let embedded = embed(svg, &source);
        // Rendering content is untouched around the metadata block.
        assert!(
            embedded
                .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\">")
        );
        assert!(embedded.ends_with("<rect/>\n</svg>"));

        let back = extract(&embedded).unwrap().expect("source found");
        assert_eq!(back.kind, SourceKind::Cast);
        assert_eq!(back.data, source.data);
        assert_eq!(back.options, source.options);
    }

    #[test]
    fn embedding_is_deterministic() {
        let svg = "<svg viewBox=\"0 0 1 1\"></svg>";
        assert_eq!(embed(svg, &sample()), embed(svg, &sample()));
    }

    #[test]
    fn foreign_svg_extracts_none() {
        assert!(extract("<svg><rect/></svg>").unwrap().is_none());
        assert!(extract("not svg at all").unwrap().is_none());
    }

    #[test]
    fn options_with_xml_hostile_title_survive() {
        let mut source = sample();
        source.kind = SourceKind::Ansi;
        source.options.title = Some(r#"a <b> & "c" &lt;"#.into());
        let embedded = embed("<svg></svg>", &source);
        let back = extract(&embedded).unwrap().unwrap();
        assert_eq!(back.options.title.as_deref(), Some(r#"a <b> & "c" &lt;"#));
        assert_eq!(back.kind, SourceKind::Ansi);
    }

    #[test]
    fn truncated_block_is_an_error_not_none() {
        let embedded = embed("<svg></svg>", &sample());
        let truncated = &embedded[..embedded.len() - 30];
        assert!(extract(truncated).is_err());
    }
}
