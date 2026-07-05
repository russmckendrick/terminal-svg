pub mod input;
pub mod pty;

use anyhow::Result;
use std::path::PathBuf;

/// Where the terminal bytes come from.
pub enum Source {
    Stdin,
    File(PathBuf),
    /// Command + args to spawn in a PTY.
    Command(Vec<String>),
}

pub struct Capture {
    pub bytes: Vec<u8>,
    /// Title derived from the source (command string), if any.
    pub title: Option<String>,
}

pub fn capture(source: Source, cols: u16, rows: u16, timeout: Option<u64>) -> Result<Capture> {
    match source {
        Source::Stdin => Ok(Capture {
            bytes: input::read_stdin()?,
            title: None,
        }),
        Source::File(path) => Ok(Capture {
            bytes: input::read_file(&path)?,
            title: None,
        }),
        Source::Command(command) => {
            let bytes = pty::run(&command, cols, rows, timeout)?;
            Ok(Capture {
                bytes,
                title: Some(command.join(" ")),
            })
        }
    }
}
