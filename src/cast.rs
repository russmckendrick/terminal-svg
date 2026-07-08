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
    /// Any other code ("i" input, "m" markers, "x" exit, future codes):
    /// nothing renderable, but kept so `edit` and `write` are lossless.
    Other { code: String, data: String },
}

impl EventData {
    /// Build from the `(code, data)` pair an event line stores.
    pub fn from_wire(code: String, data: String) -> Result<EventData> {
        Ok(match code.as_str() {
            "o" => EventData::Output(data),
            "r" => {
                let (cols, rows) = data
                    .split_once('x')
                    .and_then(|(c, r)| Some((c.parse().ok()?, r.parse().ok()?)))
                    .with_context(|| format!("malformed resize payload {data:?}"))?;
                EventData::Resize { cols, rows }
            }
            _ => EventData::Other { code, data },
        })
    }

    /// Flatten back to the `(code, data)` pair an event line stores.
    pub fn to_wire(&self) -> (&str, String) {
        match self {
            EventData::Output(data) => ("o", data.clone()),
            EventData::Resize { cols, rows } => ("r", format!("{cols}x{rows}")),
            EventData::Other { code, data } => (code.as_str(), data.clone()),
        }
    }
}

/// Read a .cast file. Every event is kept — renderers ignore the
/// non-renderable codes ("i", "m", "x", …), but they survive an
/// edit/write round trip.
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
        let time = if relative {
            clock += time.max(0.0);
            clock
        } else {
            time
        };
        events.push(Event {
            time,
            data: EventData::from_wire(code, data)?,
        });
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

/// Write a parsed cast back out, preserving its version: v2 events keep
/// absolute times, v3 events convert back to intervals (and the header
/// carries the embedded theme again). v3 comment lines are not retained —
/// `parse` drops them.
pub fn write(mut out: impl Write, header: &Header, events: &[Event]) -> Result<()> {
    match header.version {
        3 => write_v3_header(&mut out, header)?,
        _ => {
            serde_json::to_writer(&mut out, header)?;
            out.write_all(b"\n")?;
        }
    }

    let relative = header.version == 3;
    let mut clock = 0.0_f64;
    for event in events {
        let time = if relative {
            let interval = (event.time - clock).max(0.0);
            clock = event.time;
            interval
        } else {
            event.time
        };
        let (code, data) = event.data.to_wire();
        writeln!(
            out,
            "[{time:.6}, {}, {}]",
            serde_json::to_string(code)?,
            serde_json::to_string(&data)?
        )?;
    }
    Ok(())
}

fn write_v3_header(out: &mut impl Write, header: &Header) -> Result<()> {
    #[derive(Serialize)]
    struct V3ThemeOut {
        fg: String,
        bg: String,
        palette: String,
    }
    #[derive(Serialize)]
    struct V3TermOut {
        cols: usize,
        rows: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        theme: Option<V3ThemeOut>,
    }
    #[derive(Serialize)]
    struct V3HeaderOut<'a> {
        version: u8,
        term: V3TermOut,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<&'a String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        env: Option<&'a BTreeMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        idle_time_limit: Option<f64>,
    }

    let theme = header.theme.as_ref().map(|t| V3ThemeOut {
        fg: t.fg.hex(),
        bg: t.bg.hex(),
        palette: t.palette.iter().map(Rgb::hex).collect::<Vec<_>>().join(":"),
    });
    let h = V3HeaderOut {
        version: 3,
        term: V3TermOut {
            cols: header.width,
            rows: header.height,
            theme,
        },
        timestamp: header.timestamp,
        title: header.title.as_ref(),
        env: header.env.as_ref(),
        idle_time_limit: header.idle_time_limit,
    };
    serde_json::to_writer(&mut *out, &h)?;
    out.write_all(b"\n")?;
    Ok(())
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
        // "i" and "m" events are kept (renderers ignore them).
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].data, EventData::Output("hello ".into()));
        assert_eq!(
            events[1].data,
            EventData::Other {
                code: "i".into(),
                data: "typed".into()
            }
        );
        assert_eq!(events[2].time, 1.0);
        assert_eq!(
            events[3].data,
            EventData::Other {
                code: "m".into(),
                data: "marker".into()
            }
        );
        assert_eq!(
            events[4].data,
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
        // Every event is kept with its interval folded into the absolute
        // clock; comment lines are ignored.
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].time, 0.1);
        assert_eq!(events[0].data, EventData::Output("hello ".into()));
        assert_eq!(events[1].time, 0.5);
        assert_eq!(
            events[1].data,
            EventData::Other {
                code: "i".into(),
                data: "typed".into()
            }
        );
        assert_eq!(events[2].time, 1.0);
        assert_eq!(events[2].data, EventData::Output("world\r\n".into()));
        assert_eq!(events[3].time, 1.5);
        assert_eq!(events[4].time, 2.0);
        assert_eq!(
            events[4].data,
            EventData::Resize {
                cols: 100,
                rows: 30
            }
        );
        assert_eq!(events[5].time, 2.5);
        assert_eq!(
            events[5].data,
            EventData::Other {
                code: "x".into(),
                data: "0".into()
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
    fn write_round_trips_v2_and_v3() {
        for sample in [sample(), sample_v3()] {
            let (header, events) = parse(sample.as_bytes()).unwrap();
            let mut buf = Vec::new();
            write(&mut buf, &header, &events).unwrap();
            let (header2, events2) = parse(&buf[..]).unwrap();

            assert_eq!(header2.version, header.version);
            assert_eq!(
                (header2.width, header2.height),
                (header.width, header.height)
            );
            assert_eq!(header2.title, header.title);
            assert_eq!(events2.len(), events.len());
            for (a, b) in events.iter().zip(&events2) {
                assert!(
                    (a.time - b.time).abs() < 1e-6,
                    "time drift: {} vs {}",
                    a.time,
                    b.time
                );
                assert_eq!(a.data, b.data);
            }
        }

        // The v3 header keeps its embedded theme through a rewrite.
        let (header, events) = parse(sample_v3().as_bytes()).unwrap();
        let mut buf = Vec::new();
        write(&mut buf, &header, &events).unwrap();
        let (header2, _) = parse(&buf[..]).unwrap();
        let (t, t2) = (header.theme.unwrap(), header2.theme.unwrap());
        assert_eq!(t2.fg, t.fg);
        assert_eq!(t2.bg, t.bg);
        assert_eq!(t2.palette, t.palette);
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
