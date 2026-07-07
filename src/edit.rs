//! Cast cleanup: redact secrets, cut time ranges, clamp pauses.
//!
//! `terminal-svg edit` fixes a recording without re-recording it — mask a
//! token that echoed to the screen, drop a fumbled stretch, or bake an
//! idle cap into the file. Operations work on the parsed event list and
//! write back through `cast::write`, so v2 stays v2 and v3 stays v3.

use anyhow::{Context, Result};
use regex_lite::Regex;

use crate::cast::{Event, EventData, Header};

#[derive(Debug, Default)]
pub struct EditOps {
    /// Patterns whose matches are masked with `*` (same character count,
    /// so layout is unchanged).
    pub redact: Vec<String>,
    /// Time ranges to remove, in raw recording seconds; later events
    /// shift left to close each gap.
    pub cuts: Vec<(f64, f64)>,
    /// Clamp gaps between events to this many seconds, baked into the
    /// event times (unlike --idle-time-limit, which only affects a
    /// render).
    pub max_pause: Option<f64>,
}

#[derive(Debug, Default, PartialEq)]
pub struct EditStats {
    /// Individual regex matches masked.
    pub redactions: usize,
    /// Events removed by cuts.
    pub events_cut: usize,
    /// Seconds removed from the timeline (cuts and clamped pauses).
    pub time_removed: f64,
}

/// Apply the operations in order: cut (on the raw timeline the user saw),
/// then redact, then clamp pauses.
pub fn apply(header: &mut Header, events: &mut Vec<Event>, ops: &EditOps) -> Result<EditStats> {
    let mut stats = EditStats::default();

    let mut cuts = ops.cuts.clone();
    cuts.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (from, to) in cuts {
        let before = events.len();
        events.retain(|e| e.time <= from || e.time > to);
        stats.events_cut += before - events.len();
        let shift = to - from;
        for e in events.iter_mut() {
            if e.time > from {
                e.time -= shift;
            }
        }
        stats.time_removed += shift;
    }

    let patterns: Vec<Regex> = ops
        .redact
        .iter()
        .map(|p| Regex::new(p).with_context(|| format!("invalid --redact pattern {p:?}")))
        .collect::<Result<_>>()?;
    if !patterns.is_empty() {
        // Output and input are redacted as separate streams: a secret
        // echoed to the screen is matched across event boundaries; one
        // pasted into the terminal is matched in the input stream. A
        // secret typed keystroke-by-keystroke lands as one event per
        // character in the input stream and still matches concatenated.
        stats.redactions += redact_stream(events, &patterns, |data| {
            matches!(data, EventData::Output(_))
        });
        stats.redactions += redact_stream(
            events,
            &patterns,
            |data| matches!(data, EventData::Other { code, .. } if code == "i"),
        );
    }

    if let Some(max_pause) = ops.max_pause {
        let max_pause = max_pause.max(0.0);
        let mut clock = 0.0_f64; // previous event's original time
        let mut removed = 0.0_f64;
        for e in events.iter_mut() {
            let gap = (e.time - clock).max(0.0);
            clock = e.time;
            removed += (gap - max_pause).max(0.0);
            e.time -= removed;
        }
        stats.time_removed += removed;
        // The cap is baked into the times now; a stale header limit would
        // squeeze the pauses twice on render.
        header.idle_time_limit = None;
    }

    Ok(stats)
}

/// Redact regex matches across the concatenation of all events selected
/// by `select`, mapping match ranges back through per-event boundaries so
/// a secret split across events is still caught. Masked characters become
/// `*`, one per character, preserving layout and timing.
fn redact_stream(
    events: &mut [Event],
    patterns: &[Regex],
    select: impl Fn(&EventData) -> bool,
) -> usize {
    let selected: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| select(&e.data))
        .map(|(i, _)| i)
        .collect();

    // Concatenated stream + each event's byte range within it.
    let mut stream = String::new();
    let mut spans = Vec::with_capacity(selected.len());
    for &i in &selected {
        let text = event_text(&events[i].data);
        spans.push((stream.len(), stream.len() + text.len()));
        stream.push_str(text);
    }

    let mut matches: Vec<(usize, usize)> = patterns
        .iter()
        .flat_map(|re| re.find_iter(&stream).map(|m| (m.start(), m.end())))
        .collect();
    let count = matches.len();
    if count == 0 {
        return 0;
    }
    // Merge overlaps so two patterns hitting the same text mask it once.
    matches.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in matches {
        match merged.last_mut() {
            Some((_, prev_end)) if start <= *prev_end => *prev_end = (*prev_end).max(end),
            _ => merged.push((start, end)),
        }
    }

    for (slot, &i) in selected.iter().enumerate() {
        let (seg_start, seg_end) = spans[slot];
        let text = event_text(&events[i].data);
        let mut masked = String::with_capacity(text.len());
        let mut cursor = 0; // local byte offset
        for &(m_start, m_end) in &merged {
            let start = m_start.clamp(seg_start, seg_end) - seg_start;
            let end = m_end.clamp(seg_start, seg_end) - seg_start;
            if start >= end {
                continue;
            }
            masked.push_str(&text[cursor..start]);
            masked.extend(text[start..end].chars().map(|_| '*'));
            cursor = end;
        }
        if cursor == 0 {
            continue;
        }
        masked.push_str(&text[cursor..]);
        set_event_text(&mut events[i].data, masked);
    }
    count
}

fn event_text(data: &EventData) -> &str {
    match data {
        EventData::Output(text) => text,
        EventData::Other { data, .. } => data,
        EventData::Resize { .. } => unreachable!("resize events are never selected"),
    }
}

fn set_event_text(data: &mut EventData, text: String) {
    match data {
        EventData::Output(t) => *t = text,
        EventData::Other { data, .. } => *data = text,
        EventData::Resize { .. } => unreachable!("resize events are never selected"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(time: f64, data: &str) -> Event {
        Event {
            time,
            data: EventData::Output(data.into()),
        }
    }

    fn input(time: f64, data: &str) -> Event {
        Event {
            time,
            data: EventData::Other {
                code: "i".into(),
                data: data.into(),
            },
        }
    }

    fn header() -> Header {
        Header {
            version: 2,
            width: 80,
            height: 24,
            timestamp: None,
            title: None,
            env: None,
            idle_time_limit: Some(2.0),
            theme: None,
        }
    }

    fn output_text(e: &Event) -> &str {
        match &e.data {
            EventData::Output(t) => t,
            other => panic!("expected output, got {other:?}"),
        }
    }

    #[test]
    fn redacts_across_event_boundaries() {
        let mut h = header();
        let mut events = vec![out(0.1, "token: ghp_ab"), out(0.2, "cd1234 done")];
        let ops = EditOps {
            redact: vec![r"ghp_\w+".into()],
            ..Default::default()
        };
        let stats = apply(&mut h, &mut events, &ops).unwrap();
        assert_eq!(stats.redactions, 1);
        assert_eq!(output_text(&events[0]), "token: ******");
        assert_eq!(output_text(&events[1]), "****** done");
    }

    #[test]
    fn redacts_keystroke_split_input_events() {
        let mut h = header();
        let mut events = vec![
            input(0.1, "h"),
            input(0.2, "u"),
            input(0.3, "n"),
            input(0.4, "t"),
            input(0.5, "e"),
            input(0.6, "r"),
            input(0.7, "2"),
            out(0.8, "ok"),
        ];
        let ops = EditOps {
            redact: vec!["hunter2".into()],
            ..Default::default()
        };
        let stats = apply(&mut h, &mut events, &ops).unwrap();
        assert_eq!(stats.redactions, 1);
        for e in &events[..7] {
            let EventData::Other { data, .. } = &e.data else {
                panic!()
            };
            assert_eq!(data, "*");
        }
        assert_eq!(output_text(&events[7]), "ok");
    }

    #[test]
    fn redaction_preserves_multibyte_layout() {
        let mut h = header();
        let mut events = vec![out(0.1, "pw: héllo🔑 end")];
        let ops = EditOps {
            redact: vec!["héllo🔑".into()],
            ..Default::default()
        };
        apply(&mut h, &mut events, &ops).unwrap();
        // Six characters masked as six asterisks.
        assert_eq!(output_text(&events[0]), "pw: ****** end");
    }

    #[test]
    fn overlapping_patterns_mask_once() {
        let mut h = header();
        let mut events = vec![out(0.1, "abcdef")];
        let ops = EditOps {
            redact: vec!["abcd".into(), "cdef".into()],
            ..Default::default()
        };
        let stats = apply(&mut h, &mut events, &ops).unwrap();
        assert_eq!(stats.redactions, 2);
        assert_eq!(output_text(&events[0]), "******");
    }

    #[test]
    fn invalid_pattern_errors() {
        let ops = EditOps {
            redact: vec!["(".into()],
            ..Default::default()
        };
        assert!(apply(&mut header(), &mut vec![out(0.1, "x")], &ops).is_err());
    }

    #[test]
    fn cut_drops_events_and_closes_the_gap() {
        let mut h = header();
        let mut events = vec![out(1.0, "a"), out(5.0, "cut me"), out(10.0, "b")];
        let ops = EditOps {
            cuts: vec![(4.0, 8.0)],
            ..Default::default()
        };
        let stats = apply(&mut h, &mut events, &ops).unwrap();
        assert_eq!(stats.events_cut, 1);
        assert_eq!(stats.time_removed, 4.0);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].time, 1.0);
        assert_eq!(events[1].time, 6.0);
        assert_eq!(output_text(&events[1]), "b");
    }

    #[test]
    fn descending_cut_order_keeps_ranges_independent() {
        let mut h = header();
        let mut events = vec![out(1.0, "a"), out(5.0, "x"), out(10.0, "y"), out(15.0, "b")];
        // Both ranges name raw-timeline seconds; applying descending means
        // the first cut cannot shift the second's window.
        let ops = EditOps {
            cuts: vec![(4.0, 6.0), (9.0, 11.0)],
            ..Default::default()
        };
        let stats = apply(&mut h, &mut events, &ops).unwrap();
        assert_eq!(stats.events_cut, 2);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].time, 1.0);
        assert_eq!(events[1].time, 11.0);
    }

    #[test]
    fn max_pause_clamps_gaps_and_clears_header_limit() {
        let mut h = header();
        let mut events = vec![out(0.5, "a"), out(10.5, "b"), out(11.0, "c")];
        let ops = EditOps {
            max_pause: Some(1.5),
            ..Default::default()
        };
        let stats = apply(&mut h, &mut events, &ops).unwrap();
        assert_eq!(events[0].time, 0.5);
        assert_eq!(events[1].time, 2.0); // 10s gap clamped to 1.5
        assert_eq!(events[2].time, 2.5);
        assert!((stats.time_removed - 8.5).abs() < 1e-9);
        assert_eq!(h.idle_time_limit, None);
    }

    #[test]
    fn no_ops_is_identity() {
        let mut h = header();
        let mut events = vec![out(0.5, "a")];
        let stats = apply(&mut h, &mut events, &EditOps::default()).unwrap();
        assert_eq!(stats, EditStats::default());
        assert_eq!(events, vec![out(0.5, "a")]);
        assert_eq!(h.idle_time_limit, Some(2.0));
    }
}
