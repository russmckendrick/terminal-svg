use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::theme::Rgb;

/// Asciicast header, normalized from the v2 or v3 first line of a .cast
/// file. Serializing always writes the v2 shape (`CastWriter` records v2).
/// <https://docs.asciinema.org/manual/asciicast/v2/>
/// <https://docs.asciinema.org/manual/asciicast/v3/>
#[derive(Debug, Clone, Serialize)]
pub struct Header {
    pub version: u8,
    pub width: usize,
    pub height: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_time_limit: Option<f64>,
    /// Color theme embedded in the recording (v3 `term.theme`); rendered
    /// with `--theme auto`.
    #[serde(skip)]
    pub theme: Option<CastTheme>,
}

/// The palette a v3 recording carries: the terminal's colors at record time.
#[derive(Debug, Clone)]
pub struct CastTheme {
    pub fg: Rgb,
    pub bg: Rgb,
    /// 8 or 16 entries (an 8-entry palette has no bright variants).
    pub palette: Vec<Rgb>,
}

#[derive(Deserialize)]
struct V2Header {
    version: u8,
    width: usize,
    height: usize,
    #[serde(default)]
    timestamp: Option<u64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    idle_time_limit: Option<f64>,
}

#[derive(Deserialize)]
struct V3Header {
    version: u8,
    term: V3Term,
    #[serde(default)]
    timestamp: Option<u64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    idle_time_limit: Option<f64>,
}

#[derive(Deserialize)]
struct V3Term {
    cols: usize,
    rows: usize,
    #[serde(default)]
    theme: Option<V3Theme>,
}

#[derive(Deserialize)]
struct V3Theme {
    fg: Rgb,
    bg: Rgb,
    /// Colon-separated hex colors.
    palette: String,
}

/// One event line: absolute time in seconds since session start.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub time: f64,
    pub data: EventData,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventData {
    /// "o" — bytes written to the terminal.
    Output(String),
    /// "r" — terminal resized ("COLSxROWS" payload).
    Resize { cols: usize, rows: usize },
}

/// Read a .cast file. Event codes other than "o" and "r" ("i" input, "m"
/// markers, future codes) are skipped — they carry nothing renderable.
pub fn read(path: &Path) -> Result<(Header, Vec<Event>)> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    parse(BufReader::new(file)).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn parse(reader: impl BufRead) -> Result<(Header, Vec<Event>)> {
    let mut lines = reader.lines();
    let first = loop {
        match lines.next() {
            Some(line) => {
                let line = line?;
                if !line.trim().is_empty() {
                    break line;
                }
            }
            None => bail!("empty cast file"),
        }
    };
    let header = parse_header(&first)?;

    // v3 event times are intervals since the previous event; v2 times are
    // absolute. Normalize to absolute so nothing downstream cares.
    let relative = header.version == 3;
    let mut clock = 0.0_f64;
    let mut events = Vec::new();
    for line in lines {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || (relative && trimmed.starts_with('#')) {
            continue;
        }
        let (time, code, data): (f64, String, String) =
            serde_json::from_str(trimmed).context("malformed event line")?;
        // The clock advances before the code filter: a skipped "m"/"x"
        // event still carries an interval.
        let time = if relative {
            clock += time.max(0.0);
            clock
        } else {
            time
        };
        let data = match code.as_str() {
            "o" => EventData::Output(data),
            "r" => {
                let (cols, rows) = data
                    .split_once('x')
                    .and_then(|(c, r)| Some((c.parse().ok()?, r.parse().ok()?)))
                    .with_context(|| format!("malformed resize payload {data:?}"))?;
                EventData::Resize { cols, rows }
            }
            _ => continue,
        };
        events.push(Event { time, data });
    }
    Ok((header, events))
}

fn parse_header(line: &str) -> Result<Header> {
    #[derive(Deserialize)]
    struct Probe {
        version: u8,
    }
    let probe: Probe = serde_json::from_str(line).context("malformed asciicast header")?;
    match probe.version {
        2 => {
            let h: V2Header =
                serde_json::from_str(line).context("malformed asciicast v2 header")?;
            Ok(Header {
                version: h.version,
                width: h.width,
                height: h.height,
                timestamp: h.timestamp,
                title: h.title,
                env: h.env,
                idle_time_limit: h.idle_time_limit,
                theme: None,
            })
        }
        3 => {
            let h: V3Header =
                serde_json::from_str(line).context("malformed asciicast v3 header")?;
            let theme = h.term.theme.map(parse_cast_theme).transpose()?;
            Ok(Header {
                version: h.version,
                width: h.term.cols,
                height: h.term.rows,
                timestamp: h.timestamp,
                title: h.title,
                env: h.env,
                idle_time_limit: h.idle_time_limit,
                theme,
            })
        }
        v => bail!("unsupported asciicast version {v}"),
    }
}

fn parse_cast_theme(raw: V3Theme) -> Result<CastTheme> {
    let palette: Vec<Rgb> = raw
        .palette
        .split(':')
        .map(Rgb::parse)
        .collect::<Result<_>>()
        .context("malformed theme palette")?;
    if palette.len() != 8 && palette.len() != 16 {
        bail!(
            "malformed theme palette: {} colors; expected 8 or 16",
            palette.len()
        );
    }
    Ok(CastTheme {
        fg: raw.fg,
        bg: raw.bg,
        palette,
    })
}

/// Whether a path should be treated as an asciicast recording: a .cast
/// extension, or a first line that parses as an asciicast header (covers
/// downloaded recordings named .json etc.).
pub fn looks_like_cast(path: &Path) -> bool {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cast"))
    {
        return true;
    }
    first_line(path).is_some_and(|line| parse_header(&line).is_ok())
}

fn first_line(path: &Path) -> Option<String> {
    // Header lines are small; 8 KiB is generous and bounds the sniff.
    let mut buf = vec![0u8; 8192];
    let n = File::open(path).ok()?.read(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf[..n]);
    Some(text.lines().next()?.to_string())
}

/// Streaming asciicast writer used while recording.
pub struct CastWriter<W: Write> {
    out: W,
}

impl CastWriter<BufWriter<File>> {
    pub fn create(path: &Path, header: &Header) -> Result<Self> {
        let file =
            File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
        Self::new(BufWriter::new(file), header)
    }
}

impl<W: Write> CastWriter<W> {
    pub fn new(out: W, header: &Header) -> Result<Self> {
        let mut writer = Self { out };
        serde_json::to_writer(&mut writer.out, header)?;
        writer.out.write_all(b"\n")?;
        Ok(writer)
    }

    pub fn output(&mut self, elapsed: f64, data: &[u8]) -> Result<()> {
        self.event(elapsed, "o", &String::from_utf8_lossy(data))
    }

    pub fn resize(&mut self, elapsed: f64, cols: u16, rows: u16) -> Result<()> {
        self.event(elapsed, "r", &format!("{cols}x{rows}"))
    }

    fn event(&mut self, elapsed: f64, code: &str, data: &str) -> Result<()> {
        writeln!(
            self.out,
            "[{elapsed:.6}, \"{code}\", {}]",
            serde_json::to_string(data)?
        )?;
        Ok(())
    }

    /// Flush buffered events; recordings survive even if rendering fails.
    pub fn finish(mut self) -> Result<()> {
        self.out.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        concat!(
            r#"{"version": 2, "width": 80, "height": 24, "timestamp": 1750000000, "title": "demo"}"#,
            "\n",
            r#"[0.1, "o", "hello "]"#,
            "\n",
            r#"[0.5, "i", "typed"]"#,
            "\n",
            r#"[1.0, "o", "world\r\n"]"#,
            "\n",
            r#"[1.5, "m", "marker"]"#,
            "\n",
            r#"[2.0, "r", "100x30"]"#,
            "\n",
        )
    }

    #[test]
    fn parses_header_and_events() {
        let (header, events) = parse(sample().as_bytes()).unwrap();
        assert_eq!(header.version, 2);
        assert_eq!((header.width, header.height), (80, 24));
        assert_eq!(header.title.as_deref(), Some("demo"));
        // "i" and "m" events are skipped
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data, EventData::Output("hello ".into()));
        assert_eq!(events[1].time, 1.0);
        assert_eq!(
            events[2].data,
            EventData::Resize {
                cols: 100,
                rows: 30
            }
        );
    }

    fn sample_v3() -> &'static str {
        concat!(
            r##"{"version": 3, "term": {"cols": 80, "rows": 24, "type": "xterm-256color", "theme": {"fg": "#f8f8f2", "bg": "#282a36", "palette": "#000:#f00:#0f0:#ff0:#00f:#f0f:#0ff:#fff"}}, "title": "demo"}"##,
            "\n",
            r#"[0.1, "o", "hello "]"#,
            "\n",
            "# a comment line\n",
            r#"[0.4, "i", "typed"]"#,
            "\n",
            r#"[0.5, "o", "world\r\n"]"#,
            "\n",
            r#"[0.5, "m", "marker"]"#,
            "\n",
            r#"[0.5, "r", "100x30"]"#,
            "\n",
            r#"[0.5, "x", "0"]"#,
            "\n",
        )
    }

    #[test]
    fn parses_v3_with_relative_times() {
        let (header, events) = parse(sample_v3().as_bytes()).unwrap();
        assert_eq!(header.version, 3);
        assert_eq!((header.width, header.height), (80, 24));
        assert_eq!(header.title.as_deref(), Some("demo"));
        // "i"/"m"/"x" are skipped, but their intervals still advance the
        // clock; comment lines are ignored.
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].time, 0.1);
        assert_eq!(events[0].data, EventData::Output("hello ".into()));
        assert_eq!(events[1].time, 1.0);
        assert_eq!(events[1].data, EventData::Output("world\r\n".into()));
        assert_eq!(events[2].time, 2.0);
        assert_eq!(
            events[2].data,
            EventData::Resize {
                cols: 100,
                rows: 30
            }
        );
    }

    #[test]
    fn parses_v3_theme() {
        let (header, _) = parse(sample_v3().as_bytes()).unwrap();
        let theme = header.theme.unwrap();
        assert_eq!(theme.fg, Rgb::new(0xf8, 0xf8, 0xf2));
        assert_eq!(theme.bg, Rgb::new(0x28, 0x2a, 0x36));
        assert_eq!(theme.palette.len(), 8);
        assert_eq!(theme.palette[1], Rgb::new(0xff, 0, 0));

        // 16-entry palettes parse too; other lengths are malformed.
        let full = "#000:".repeat(15) + "#000";
        let line = format!(
            r##"{{"version": 3, "term": {{"cols": 1, "rows": 1, "theme": {{"fg": "#fff", "bg": "#000", "palette": "{full}"}}}}}}"##
        );
        assert_eq!(
            parse_header(&line).unwrap().theme.unwrap().palette.len(),
            16
        );
        let bad = r##"{"version": 3, "term": {"cols": 1, "rows": 1, "theme": {"fg": "#fff", "bg": "#000", "palette": "#000:#111"}}}"##;
        assert!(parse_header(bad).is_err());
    }

    #[test]
    fn v3_negative_intervals_clamp() {
        let cast = concat!(
            r#"{"version": 3, "term": {"cols": 80, "rows": 24}}"#,
            "\n",
            r#"[1.0, "o", "a"]"#,
            "\n",
            r#"[-5.0, "o", "b"]"#,
            "\n",
        );
        let (_, events) = parse(cast.as_bytes()).unwrap();
        assert_eq!(events[0].time, 1.0);
        assert_eq!(events[1].time, 1.0);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&b""[..]).is_err());
        assert!(parse(&b"not json\n"[..]).is_err());
        let v4 = r#"{"version": 4, "term": {"cols": 80, "rows": 24}}"#;
        assert!(parse(v4.as_bytes()).is_err());
        // A v3 header without the term object is malformed.
        let v3 = r#"{"version": 3, "width": 80, "height": 24}"#;
        assert!(parse(v3.as_bytes()).is_err());
    }

    #[test]
    fn header_ignores_unknown_fields() {
        let line = r##"{"version": 2, "width": 1, "height": 1, "theme": {"fg": "#fff"}}"##;
        assert!(parse_header(line).is_ok());
    }

    #[test]
    fn writer_roundtrips() {
        let header = Header {
            version: 2,
            width: 80,
            height: 24,
            timestamp: None,
            title: Some("t".into()),
            env: None,
            idle_time_limit: None,
            theme: None,
        };
        let mut buf = Vec::new();
        let mut w = CastWriter::new(&mut buf, &header).unwrap();
        w.output(0.25, "a\x1b[31mb\x1b[0m\r\n\"quoted\"".as_bytes())
            .unwrap();
        w.resize(1.75, 120, 40).unwrap();
        w.finish().unwrap();

        let (header2, events) = parse(&buf[..]).unwrap();
        assert_eq!(header2.title.as_deref(), Some("t"));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].time, 0.25);
        assert_eq!(
            events[0].data,
            EventData::Output("a\x1b[31mb\x1b[0m\r\n\"quoted\"".into())
        );
        assert_eq!(
            events[1].data,
            EventData::Resize {
                cols: 120,
                rows: 40
            }
        );
    }

    #[test]
    fn detects_cast_by_extension_and_content() {
        assert!(looks_like_cast(Path::new("session.cast")));
        assert!(looks_like_cast(Path::new("SESSION.CAST")));
        assert!(!looks_like_cast(Path::new("missing.ansi")));

        let dir = std::env::temp_dir();
        let sniffed = dir.join("terminal-svg-test-sniff.json");
        std::fs::write(&sniffed, sample()).unwrap();
        assert!(looks_like_cast(&sniffed));
        let plain = dir.join("terminal-svg-test-plain.txt");
        std::fs::write(&plain, "hello\nworld\n").unwrap();
        assert!(!looks_like_cast(&plain));
        let _ = std::fs::remove_file(&sniffed);
        let _ = std::fs::remove_file(&plain);
    }
}
