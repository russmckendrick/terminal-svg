//! Interactive session recording for `terminal-svg rec`: spawn a shell on a
//! PTY, mirror it to the user's terminal, and stream timestamped output
//! into an asciicast. Thin plumbing only — all timing/rendering logic lives
//! in `anim` and `render`.

use std::path::Path;

use anyhow::Result;

use crate::cast::Header;

pub struct RecordOptions {
    /// Command to record; empty means $SHELL.
    pub command: Vec<String>,
    /// Explicit size; defaults to the invoking terminal's.
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[cfg(unix)]
pub use unix::record;

#[cfg(not(unix))]
pub fn record(_cast_path: &Path, _opts: &RecordOptions) -> Result<Header> {
    anyhow::bail!("terminal-svg rec is not supported on this platform yet (Unix only)");
}

#[cfg(unix)]
mod unix {
    use super::*;

    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use anyhow::{Context, bail};
    use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
    use rustix::event::{PollFd, PollFlags, Timespec};
    use rustix::termios::{self, OptionalActions, Termios};

    use crate::cast::CastWriter;

    type SharedWriter = Arc<Mutex<CastWriter<std::io::BufWriter<std::fs::File>>>>;

    /// How often the input pump wakes to check for shutdown and window
    /// resizes. Invisible in a replay; SIGWINCH plumbing isn't worth it.
    const POLL_TICK: Timespec = Timespec {
        tv_sec: 0,
        tv_nsec: 100_000_000,
    };

    /// Restores the invoking terminal's mode when dropped, so every exit
    /// path out of the session (including `?` unwinds) leaves it sane.
    struct RawModeGuard {
        original: Termios,
    }

    impl RawModeGuard {
        fn new() -> Result<Self> {
            let stdin = std::io::stdin();
            let original = termios::tcgetattr(&stdin).context("failed to read terminal mode")?;
            let mut raw = original.clone();
            raw.make_raw();
            termios::tcsetattr(&stdin, OptionalActions::Now, &raw)
                .context("failed to enter raw mode")?;
            Ok(Self { original })
        }
    }

    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            let _ = termios::tcsetattr(std::io::stdin(), OptionalActions::Now, &self.original);
        }
    }

    /// Run the interactive session, streaming events into `cast_path`.
    /// Returns the header (for reuse when rendering).
    pub fn record(cast_path: &Path, opts: &RecordOptions) -> Result<Header> {
        if !termios::isatty(std::io::stdin()) {
            bail!("rec needs an interactive terminal (stdin is not a tty)");
        }

        let command = if opts.command.is_empty() {
            vec![std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string())]
        } else {
            opts.command.clone()
        };

        let (cur_cols, cur_rows) = winsize().unwrap_or((80, 24));
        let cols = opts.cols.unwrap_or(cur_cols);
        let rows = opts.rows.unwrap_or(cur_rows);

        let header = Header {
            version: 2,
            width: cols as usize,
            height: rows as usize,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs()),
            title: (!opts.command.is_empty()).then(|| opts.command.join(" ")),
            env: Some(BTreeMap::from([
                ("TERM".to_string(), "xterm-256color".to_string()),
                (
                    "SHELL".to_string(),
                    std::env::var("SHELL").unwrap_or_default(),
                ),
            ])),
            idle_time_limit: None,
            theme: None,
        };
        let writer: SharedWriter = Arc::new(Mutex::new(CastWriter::create(cast_path, &header)?));

        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| anyhow::anyhow!("failed to open pty: {e}"))?;

        let (program, args) = command.split_first().expect("command is never empty");
        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| anyhow::anyhow!("failed to spawn {program:?}: {e}"))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| anyhow::anyhow!("failed to open pty reader: {e}"))?;
        let pty_writer = pair
            .master
            .take_writer()
            .map_err(|e| anyhow::anyhow!("failed to open pty writer: {e}"))?;

        eprintln!(
            "recording to {} — exit the session to finish",
            cast_path.display()
        );

        let start = Instant::now();
        let shutdown = Arc::new(AtomicBool::new(false));

        // Raw mode last: every failure before this point leaves the
        // terminal untouched, and the guard restores it from here on.
        let guard = RawModeGuard::new()?;

        // The input pump owns the master (MasterPty is Send, not Sync):
        // it forwards keystrokes, applies window resizes, and drops the
        // master when it exits.
        let input_pump = {
            let writer = Arc::clone(&writer);
            let shutdown = Arc::clone(&shutdown);
            let master = pair.master;
            std::thread::spawn(move || {
                input_loop(master, pty_writer, writer, shutdown, start, (cols, rows));
            })
        };

        // Output pump on this thread: mirror the session to the user's
        // terminal and log it. EOF/EIO on the master reader = child gone.
        let pumped: Result<()> = (|| {
            let mut stdout = std::io::stdout().lock();
            let mut buf = [0u8; 32 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => return Ok(()),
                    Ok(n) => {
                        stdout.write_all(&buf[..n])?;
                        stdout.flush()?;
                        let elapsed = start.elapsed().as_secs_f64();
                        if let Ok(mut w) = writer.lock() {
                            w.output(elapsed, &buf[..n])?;
                        }
                    }
                }
            }
        })();

        shutdown.store(true, Ordering::Relaxed);
        let _ = child.wait();
        let _ = input_pump.join();
        drop(guard);
        pumped?;

        let writer = Arc::into_inner(writer)
            .context("cast writer still shared")?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("cast writer poisoned"))?;
        writer.finish()?;
        eprintln!("recording saved to {}", cast_path.display());
        Ok(header)
    }

    /// Forward keystrokes to the PTY and watch for window resizes until
    /// shutdown. Runs on its own thread; wakes every POLL_TICK.
    fn input_loop(
        master: Box<dyn MasterPty + Send>,
        mut pty_writer: Box<dyn Write + Send>,
        writer: SharedWriter,
        shutdown: Arc<AtomicBool>,
        start: Instant,
        initial_size: (u16, u16),
    ) {
        let stdin = std::io::stdin();
        let mut size = initial_size;
        let mut buf = [0u8; 4096];

        while !shutdown.load(Ordering::Relaxed) {
            let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
            let readable =
                matches!(rustix::event::poll(&mut fds, Some(&POLL_TICK)), Ok(n) if n > 0);

            if readable {
                match rustix::io::read(&stdin, &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if pty_writer.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        let _ = pty_writer.flush();
                    }
                }
            }

            if let Some(new_size) = winsize().filter(|s| *s != size) {
                size = new_size;
                let _ = master.resize(PtySize {
                    rows: new_size.1,
                    cols: new_size.0,
                    pixel_width: 0,
                    pixel_height: 0,
                });
                if let Ok(mut w) = writer.lock() {
                    let _ = w.resize(start.elapsed().as_secs_f64(), new_size.0, new_size.1);
                }
            }
        }
    }

    fn winsize() -> Option<(u16, u16)> {
        let size = termios::tcgetwinsize(std::io::stdout()).ok()?;
        (size.ws_col > 0 && size.ws_row > 0).then_some((size.ws_col, size.ws_row))
    }
}
