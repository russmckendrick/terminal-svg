use crate::cast::{Event, EventData, Header};
use crate::term::Interpreter;
use crate::term::screen::Screen;
use crate::theme::Theme;

/// Bursts of output closer together than this collapse into one frame.
const MAX_FPS: f64 = 30.0;
/// Hold on the final frame before the loop restarts.
const TRAILING_PAUSE: f64 = 1.5;
/// Idle cap applied when neither the CLI nor the cast header set one.
const DEFAULT_IDLE_LIMIT: f64 = 2.0;

#[derive(Debug, Clone)]
pub struct AnimOptions {
    /// CLI idle cap; falls back to the header's, then DEFAULT_IDLE_LIMIT.
    pub idle_time_limit: Option<f64>,
    pub speed: f64,
}

#[derive(Debug)]
pub struct Frame {
    /// Seconds (adjusted playback time) at which this frame appears.
    pub time: f64,
    pub screen: Screen,
    /// Cursor (col, row) when visible.
    pub cursor: Option<(usize, usize)>,
}

/// The replay: frames on an adjusted timeline plus the fixed canvas grid,
/// sized to the largest terminal seen (resizes render top-left inside it).
#[derive(Debug)]
pub struct Animation {
    pub frames: Vec<Frame>,
    /// Loop duration in seconds, including the trailing pause.
    pub duration: f64,
    pub cols: usize,
    pub rows: usize,
}

/// Replay cast events through a virtual terminal and collect deduplicated
/// screen keyframes on the adjusted (idle-capped, speed-scaled) timeline.
pub fn build_frames(
    header: &Header,
    events: &[Event],
    theme: &Theme,
    opts: &AnimOptions,
) -> Animation {
    let idle_limit = opts
        .idle_time_limit
        .or(header.idle_time_limit)
        .unwrap_or(DEFAULT_IDLE_LIMIT)
        .max(0.0);
    let speed = if opts.speed > 0.0 { opts.speed } else { 1.0 };

    // Canvas = the largest grid the recording ever uses.
    let (mut cols, mut rows) = (header.width, header.height);
    for event in events {
        if let EventData::Resize { cols: c, rows: r } = event.data {
            cols = cols.max(c);
            rows = rows.max(r);
        }
    }

    let mut vt = Interpreter::new(header.width, header.height);
    let mut frames: Vec<Frame> = Vec::new();
    let mut push_frame = |time: f64, vt: &Interpreter| {
        let screen = vt.snapshot(theme);
        let cursor = vt.cursor();
        // Two frames render identically when their visible content and
        // cursor match. Whole-Screen equality is too strict: a resize
        // alters `cols` and appends empty rows, but the canvas is fixed at
        // the max grid and empty rows emit nothing.
        if let Some(prev) = frames.last()
            && visible_rows(&prev.screen) == visible_rows(&screen)
            && prev.cursor == cursor
        {
            return;
        }
        frames.push(Frame {
            time,
            screen,
            cursor,
        });
    };

    // Frame 0: the empty terminal before any output.
    push_frame(0.0, &vt);

    let min_gap = 1.0 / MAX_FPS;
    let mut playback = 0.0f64; // adjusted time of the current event
    let mut prev_raw = 0.0f64;
    let mut pending_since: Option<f64> = None; // unsnapshotted feeds started here

    for event in events {
        let raw_delta = (event.time - prev_raw).max(0.0);
        prev_raw = event.time;
        let time = playback + raw_delta.min(idle_limit) / speed;

        // The burst that started at `pending_since` has settled: this event
        // lands a visible gap later, so snapshot the accumulated state at
        // the burst's start time.
        if let Some(start) = pending_since
            && time - start >= min_gap
        {
            push_frame(start, &vt);
            pending_since = None;
        }

        match &event.data {
            EventData::Output(data) => vt.feed(data),
            EventData::Resize { cols, rows } => vt.resize(*cols, *rows),
        }
        pending_since.get_or_insert(time);
        playback = time;
    }
    if let Some(start) = pending_since {
        push_frame(start, &vt);
    }

    let duration = frames.last().map_or(0.0, |f| f.time) + TRAILING_PAUSE;
    Animation {
        frames,
        duration,
        cols,
        rows,
    }
}

/// Rows up to the last non-empty one; empty rows render nothing.
fn visible_rows(screen: &Screen) -> &[Vec<crate::term::screen::StyledRun>] {
    let mut rows = &screen.rows[..];
    while rows.last().is_some_and(|row| row.is_empty()) {
        rows = &rows[..rows.len() - 1];
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin;

    fn header(width: usize, height: usize) -> Header {
        Header {
            version: 2,
            width,
            height,
            timestamp: None,
            title: None,
            env: None,
            idle_time_limit: None,
            theme: None,
        }
    }

    fn out(time: f64, data: &str) -> Event {
        Event {
            time,
            data: EventData::Output(data.into()),
        }
    }

    fn build(events: &[Event], opts: &AnimOptions) -> Animation {
        let theme = builtin::load("dracula").unwrap();
        build_frames(&header(20, 5), events, &theme, opts)
    }

    fn default_opts() -> AnimOptions {
        AnimOptions {
            idle_time_limit: None,
            speed: 1.0,
        }
    }

    #[test]
    fn distinct_events_become_frames() {
        let anim = build(&[out(0.5, "a"), out(1.5, "b")], &default_opts());
        // empty screen + "a" + "ab"
        assert_eq!(anim.frames.len(), 3);
        assert_eq!(anim.frames[0].time, 0.0);
        assert_eq!(anim.frames[1].time, 0.5);
        assert_eq!(anim.frames[2].time, 1.5);
        assert_eq!(anim.duration, 1.5 + TRAILING_PAUSE);
        assert_eq!((anim.cols, anim.rows), (20, 5));
    }

    #[test]
    fn bursts_coalesce_into_one_frame() {
        let anim = build(
            &[
                out(0.5, "a"),
                out(0.505, "b"),
                out(0.51, "c"),
                out(2.0, "!"),
            ],
            &default_opts(),
        );
        // empty + "abc" (one frame at 0.5) + "abc!"
        assert_eq!(anim.frames.len(), 3);
        assert_eq!(anim.frames[1].time, 0.5);
        assert_eq!(anim.frames[1].screen.rows[0][0].text, "abc");
        assert_eq!(anim.frames[2].time, 2.0);
    }

    #[test]
    fn idle_time_is_capped() {
        let anim = build(&[out(0.5, "a"), out(60.5, "b")], &default_opts());
        // 60s gap clamps to the 2s default limit.
        assert_eq!(anim.frames[2].time, 2.5);
    }

    #[test]
    fn speed_scales_the_timeline() {
        let opts = AnimOptions {
            idle_time_limit: None,
            speed: 2.0,
        };
        let anim = build(&[out(1.0, "a"), out(2.0, "b")], &opts);
        assert_eq!(anim.frames[1].time, 0.5);
        assert_eq!(anim.frames[2].time, 1.0);
    }

    #[test]
    fn identical_screens_dedup() {
        // Overwriting "x" with "x" changes nothing visible.
        let anim = build(&[out(0.5, "x"), out(1.5, "\rx")], &default_opts());
        assert_eq!(anim.frames.len(), 2);
    }

    #[test]
    fn resize_grows_the_canvas() {
        let events = [
            out(0.1, "a"),
            Event {
                time: 1.0,
                data: EventData::Resize { cols: 30, rows: 8 },
            },
            out(2.0, "b"),
        ];
        let anim = build(&events, &default_opts());
        assert_eq!((anim.cols, anim.rows), (30, 8));
    }

    #[test]
    fn cursor_tracks_output() {
        let anim = build(&[out(0.5, "ab")], &default_opts());
        assert_eq!(anim.frames[1].cursor, Some((2, 0)));
    }

    #[test]
    fn no_events_still_yields_the_empty_frame() {
        let anim = build(&[], &default_opts());
        assert_eq!(anim.frames.len(), 1);
        assert_eq!(anim.duration, TRAILING_PAUSE);
    }
}
