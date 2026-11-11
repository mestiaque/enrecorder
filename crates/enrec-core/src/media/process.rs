//! Child processes: always a fixed program path plus an argument array, never a shell, and
//! always with a time limit so a stuck tool cannot hang the app.

use crate::error::{AppError, Result};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A `Command` for `program` that opens no console window on Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

pub struct Finished {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `program args…`, collecting output; kills it after `timeout`.
pub fn run(program: &Path, args: &[String], timeout: Duration) -> Result<Finished> {
    let mut child = command(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Io(format!("could not start {}: {e}", program.display())))?;
    // Read both pipes on threads so a chatty process cannot block on a full pipe.
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let t_out = std::thread::spawn(move || {
        let mut s = Vec::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_end(&mut s);
        }
        s
    });
    let t_err = std::thread::spawn(move || {
        let mut s = Vec::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_end(&mut s);
        }
        s
    });
    let start = Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break Some(st);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = String::from_utf8_lossy(&t_out.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).into_owned();
    match status {
        Some(st) => Ok(Finished { success: st.success(), code: st.code(), stdout, stderr }),
        None => Err(AppError::Other(format!("{} did not finish within {} s", program.display(), timeout.as_secs()))),
    }
}

/// The last non-empty lines of a tool's stderr, for error messages.
pub fn tail(text: &str, lines: usize) -> String {
    let v: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    v[v.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_last_lines() {
        assert_eq!(tail("a\n\nb\nc\n", 2), "b\nc");
        assert_eq!(tail("", 3), "");
    }

    #[cfg(unix)]
    #[test]
    fn times_out_and_kills() {
        let r = run(Path::new("/bin/sleep"), &["5".into()], Duration::from_millis(200));
        assert!(r.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn collects_output() {
        let r = run(Path::new("/bin/sh"), &["-c".into(), "echo out; echo err >&2; exit 3".into()], Duration::from_secs(5)).unwrap();
        assert!(!r.success);
        assert_eq!(r.code, Some(3));
        assert_eq!(r.stdout.trim(), "out");
        assert_eq!(r.stderr.trim(), "err");
    }
}
