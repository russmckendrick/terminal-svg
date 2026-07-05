use std::io::Read;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

/// Run a command in a pseudo-terminal so it detects a TTY and emits full
/// color/interactive output, and capture everything it writes.
pub fn run(command: &[String], cols: u16, rows: u16, timeout: Option<u64>) -> Result<Vec<u8>> {
    let (program, args) = command.split_first().context("empty command")?;

    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| anyhow::anyhow!("failed to open pty: {e}"))?;

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
    // Close our copy of the slave so the reader sees EOF when the child
    // exits.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| anyhow::anyhow!("failed to open pty reader: {e}"))?;
    let output = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        // EIO on master read after child exit is the normal Unix EOF.
        let _ = reader.read_to_end(&mut bytes);
        bytes
    });

    let deadline = timeout.map(|secs| Instant::now() + Duration::from_secs(secs));
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| anyhow::anyhow!("failed to wait for child: {e}"))?
        {
            break Some(status);
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    // Drop the master so the reader thread's read_to_end terminates.
    drop(pair.master);
    let bytes = output
        .join()
        .map_err(|_| anyhow::anyhow!("pty reader thread panicked"))?;

    match status {
        None => eprintln!(
            "warning: command timed out after {}s; rendering output captured so far",
            timeout.unwrap_or_default()
        ),
        Some(s) if !s.success() => {
            eprintln!("warning: command exited with {s}; rendering its output anyway")
        }
        Some(_) => {}
    }
    if bytes.is_empty() {
        bail!("command produced no output");
    }
    Ok(bytes)
}
