//! The visual editor: `terminal-svg editor` serves a single-page UI on
//! 127.0.0.1 with a live preview of every render option.
//!
//! The browser is the frontend; this binary is the renderer. The page
//! POSTs a `RenderOptions` JSON and gets back the SVG the CLI would have
//! written — same pipeline, same fonts, no second implementation. State
//! is one loaded source (the same `EmbeddedSource` that round-trips
//! through SVG metadata), so opening a .cast, an .ansi dump, or a
//! terminal-svg SVG all look identical past the sniffing.

use std::sync::Mutex;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};

use crate::embed::{self, EmbeddedSource, SourceKind};
use crate::options::RenderOptions;
use crate::pipeline::{SourceInput, render_svg};
use crate::{cast, theme};

const PAGE: &str = include_str!("../assets/editor.html");

pub struct Editor {
    source: Mutex<Option<EmbeddedSource>>,
    /// Name shown in the UI (the opened file, or the dropped file).
    name: Mutex<Option<String>>,
    /// Where "Save" writes on disk.
    output: String,
    /// Seeds the controls before anything is loaded (CLI flags + config).
    initial: RenderOptions,
}

impl Editor {
    pub fn new(
        source: Option<EmbeddedSource>,
        name: Option<String>,
        output: String,
        initial: RenderOptions,
    ) -> Self {
        Editor {
            source: Mutex::new(source),
            name: Mutex::new(name),
            output,
            initial,
        }
    }
}

/// Serve the editor forever. `ready` runs once with the bound URL —
/// print it, open the browser.
pub fn serve(editor: &Editor, port: u16, ready: impl FnOnce(&str)) -> Result<()> {
    let server = tiny_http::Server::http(("127.0.0.1", port))
        .map_err(|e| anyhow!("failed to bind 127.0.0.1:{port}: {e}"))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .unwrap_or(port);
    ready(&format!("http://127.0.0.1:{port}"));

    for mut request in server.incoming_requests() {
        let (status, content_type, body) = respond(editor, &mut request);
        let response = tiny_http::Response::from_string(body)
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                    .expect("static header"),
            );
        // A dropped connection is the browser's business, not a server error.
        let _ = request.respond(response);
    }
    Ok(())
}

fn respond(editor: &Editor, request: &mut tiny_http::Request) -> (u16, &'static str, String) {
    let json = |result: Result<String>| match result {
        Ok(body) => (200, "application/json", body),
        Err(e) => (
            400,
            "application/json",
            serde_json::json!({ "error": format!("{e:#}") }).to_string(),
        ),
    };

    let mut body = String::new();
    if *request.method() == tiny_http::Method::Post
        && request.as_reader().read_to_string(&mut body).is_err()
    {
        return json(Err(anyhow!("request body is not UTF-8")));
    }

    match (request.method().as_str(), request.url()) {
        ("GET", "/") => (200, "text/html; charset=utf-8", PAGE.to_string()),
        ("GET", "/api/state") => json(state_json(editor)),
        ("POST", "/api/render") => json(handle_render(editor, &body)),
        ("POST", "/api/open") => json(handle_open(editor, &body)),
        ("POST", "/api/save") => json(handle_save(editor, &body)),
        _ => (404, "application/json", r#"{"error":"not found"}"#.into()),
    }
}

#[derive(Serialize)]
struct State {
    loaded: bool,
    kind: Option<&'static str>,
    name: Option<String>,
    options: RenderOptions,
    themes: Vec<&'static str>,
    output: String,
}

fn state_json(editor: &Editor) -> Result<String> {
    let source = editor.source.lock().unwrap();
    let state = State {
        loaded: source.is_some(),
        kind: source.as_ref().map(|s| s.kind.extension()),
        name: editor.name.lock().unwrap().clone(),
        options: source
            .as_ref()
            .map(|s| s.options.clone())
            .unwrap_or_else(|| editor.initial.clone()),
        themes: theme::builtin::names().collect(),
        output: editor.output.clone(),
    };
    Ok(serde_json::to_string(&state)?)
}

#[derive(Deserialize)]
struct RenderRequest {
    options: RenderOptions,
    /// Include the source metadata block (Download/Save); previews skip
    /// it to keep the round trips light.
    #[serde(default)]
    embed: bool,
}

/// Render the loaded source with the requested options. The source's
/// capture context (cols/rows for ANSI, the command-string title
/// fallback) fills any gap the UI left.
fn render_with(
    source: &EmbeddedSource,
    mut options: RenderOptions,
    embed_source: bool,
) -> Result<String> {
    if options.title_fallback.is_none() {
        options.title_fallback = source.options.title_fallback.clone();
    }
    if options.cols.is_none() {
        options.cols = source.options.cols;
    }
    if options.rows.is_none() {
        options.rows = source.options.rows;
    }

    let svg = match source.kind {
        SourceKind::Cast => {
            let (header, events) = cast::parse(&source.data[..])?;
            render_svg(
                &SourceInput::Cast {
                    header: &header,
                    events: &events,
                },
                &options,
            )?
        }
        SourceKind::Ansi => render_svg(
            &SourceInput::Ansi {
                bytes: &source.data,
            },
            &options,
        )?,
    };
    if !embed_source {
        return Ok(svg);
    }
    Ok(embed::embed(
        &svg,
        &EmbeddedSource {
            kind: source.kind,
            data: source.data.clone(),
            options,
        },
    ))
}

fn handle_render(editor: &Editor, body: &str) -> Result<String> {
    let req: RenderRequest = serde_json::from_str(body).context("bad render request")?;
    let source = editor.source.lock().unwrap();
    let source = source.as_ref().ok_or_else(|| anyhow!("no source loaded"))?;
    let svg = render_with(source, req.options, req.embed)?;
    Ok(serde_json::json!({ "svg": svg }).to_string())
}

#[derive(Deserialize)]
struct OpenRequest {
    name: String,
    /// File bytes, base64.
    data: String,
    /// The UI's current options, kept for cast/ANSI files; a terminal-svg
    /// SVG brings its own.
    options: RenderOptions,
}

fn handle_open(editor: &Editor, body: &str) -> Result<String> {
    let req: OpenRequest = serde_json::from_str(body).context("bad open request")?;
    let data = BASE64
        .decode(req.data.as_bytes())
        .context("file payload is not valid base64")?;
    let source = sniff_source(&req.name, data, req.options)?;

    *editor.source.lock().unwrap() = Some(source);
    *editor.name.lock().unwrap() = Some(req.name);
    state_json(editor)
}

/// Classify dropped bytes: a terminal-svg SVG (which carries its own
/// source and options), an asciicast, or raw ANSI.
pub fn sniff_source(
    name: &str,
    data: Vec<u8>,
    ui_options: RenderOptions,
) -> Result<EmbeddedSource> {
    let text = String::from_utf8_lossy(&data);
    let head = text.trim_start();
    if head.starts_with("<?xml") || head.starts_with("<svg") {
        return match embed::extract(&text)? {
            Some(source) => Ok(source),
            None => bail!(
                "{name} has no embedded terminal-svg source (rendered with \
                 --no-embed-source, or a foreign SVG?)"
            ),
        };
    }
    let kind = if head.starts_with('{') && cast::parse(head.as_bytes()).is_ok() {
        SourceKind::Cast
    } else {
        SourceKind::Ansi
    };
    Ok(EmbeddedSource {
        kind,
        data,
        options: ui_options,
    })
}

fn handle_save(editor: &Editor, body: &str) -> Result<String> {
    let req: RenderRequest = serde_json::from_str(body).context("bad save request")?;
    let source = editor.source.lock().unwrap();
    let source = source.as_ref().ok_or_else(|| anyhow!("no source loaded"))?;
    let svg = render_with(source, req.options, true)?;
    std::fs::write(&editor.output, &svg)
        .with_context(|| format!("failed to write {}", editor.output))?;
    Ok(serde_json::json!({ "path": editor.output }).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast_source() -> EmbeddedSource {
        let data = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/typing.cast"
        ))
        .unwrap();
        EmbeddedSource {
            kind: SourceKind::Cast,
            data,
            options: RenderOptions::default(),
        }
    }

    #[test]
    fn render_request_produces_svg() {
        let editor = Editor::new(
            Some(cast_source()),
            Some("typing.cast".into()),
            "out.svg".into(),
            RenderOptions::default(),
        );
        let body = r#"{"options":{"theme":"nord","static":true}}"#;
        let response = handle_render(&editor, body).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        let svg = parsed["svg"].as_str().unwrap();
        assert!(svg.starts_with("<svg"));
        // Previews skip the source block; embed:true includes it.
        assert!(!svg.contains("terminal-svg-source"));

        let body = r#"{"options":{},"embed":true}"#;
        let response = handle_render(&editor, body).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(
            parsed["svg"]
                .as_str()
                .unwrap()
                .contains("terminal-svg-source")
        );
    }

    #[test]
    fn render_without_source_is_an_error() {
        let editor = Editor::new(None, None, "out.svg".into(), RenderOptions::default());
        assert!(handle_render(&editor, r#"{"options":{}}"#).is_err());
    }

    #[test]
    fn unknown_option_keys_are_rejected() {
        let editor = Editor::new(
            Some(cast_source()),
            None,
            "out.svg".into(),
            RenderOptions::default(),
        );
        assert!(handle_render(&editor, r#"{"options":{"font-szie":16}}"#).is_err());
    }

    #[test]
    fn sniffing_classifies_all_three_kinds() {
        let opts = RenderOptions::default();

        let cast = sniff_source("a.cast", cast_source().data, opts.clone()).unwrap();
        assert_eq!(cast.kind, SourceKind::Cast);

        let ansi = sniff_source("a.txt", b"\x1b[31mhi\x1b[0m".to_vec(), opts.clone()).unwrap();
        assert_eq!(ansi.kind, SourceKind::Ansi);

        // A terminal-svg SVG brings its own source and options back.
        let inner = EmbeddedSource {
            kind: SourceKind::Cast,
            data: cast_source().data,
            options: RenderOptions {
                theme: Some("nord".into()),
                ..Default::default()
            },
        };
        let svg = embed::embed("<svg viewBox=\"0 0 1 1\"></svg>", &inner);
        let opened = sniff_source("a.svg", svg.into_bytes(), opts).unwrap();
        assert_eq!(opened.kind, SourceKind::Cast);
        assert_eq!(opened.options.theme.as_deref(), Some("nord"));

        // A foreign SVG is a clear error, not silently treated as ANSI.
        assert!(
            sniff_source(
                "f.svg",
                b"<svg><rect/></svg>".to_vec(),
                RenderOptions::default()
            )
            .is_err()
        );
    }

    #[test]
    fn state_reports_the_loaded_source() {
        let editor = Editor::new(
            Some(cast_source()),
            Some("typing.cast".into()),
            "demo.svg".into(),
            RenderOptions::default(),
        );
        let state: serde_json::Value = serde_json::from_str(&state_json(&editor).unwrap()).unwrap();
        assert_eq!(state["loaded"], true);
        assert_eq!(state["kind"], "cast");
        assert_eq!(state["name"], "typing.cast");
        assert_eq!(state["output"], "demo.svg");
        assert!(state["themes"].as_array().unwrap().len() >= 9);
    }
}
