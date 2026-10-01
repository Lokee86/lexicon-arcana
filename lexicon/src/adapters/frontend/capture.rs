use std::{
    io::Read,
    process::{Child, ChildStderr},
    thread::{self, JoinHandle},
};

use crate::adapters::AdapterError;

const MAX_STDERR_BYTES: usize = 512 * 1024;

#[derive(Default)]
pub(crate) struct StderrCapture {
    bytes: Vec<u8>,
    truncated: bool,
}

pub(crate) fn capture_stderr(stderr: ChildStderr) -> JoinHandle<StderrCapture> {
    thread::spawn(move || read_stderr(stderr))
}

pub(crate) fn terminate(
    child: &mut Child,
    stderr_thread: JoinHandle<StderrCapture>,
    message: String,
) -> AdapterError {
    let _ = child.kill();
    let _ = child.wait();
    let stderr = stderr_thread.join().unwrap_or_default();
    AdapterError::new(format!("{message}{}", stderr_suffix(&stderr)))
}

pub(crate) fn stderr_suffix(stderr: &StderrCapture) -> String {
    if stderr.bytes.is_empty() {
        return String::new();
    }
    let mut text = String::from_utf8_lossy(&stderr.bytes).trim().to_owned();
    if stderr.truncated {
        text.push_str(" [truncated]");
    }
    format!(": {text}")
}

pub(crate) fn replay_stderr(stderr: &StderrCapture) {
    if stderr.bytes.is_empty() {
        return;
    }
    eprint!("{}", String::from_utf8_lossy(&stderr.bytes));
    if stderr.truncated {
        eprintln!("[lexicon-perf] frontend_stderr_truncated=1");
    } else if !stderr.bytes.ends_with(b"\n") {
        eprintln!();
    }
}

fn read_stderr(mut stderr: ChildStderr) -> StderrCapture {
    let mut result = StderrCapture::default();
    let mut buffer = [0_u8; 4096];
    while let Ok(count) = stderr.read(&mut buffer) {
        if count == 0 {
            break;
        }
        let remaining = MAX_STDERR_BYTES.saturating_sub(result.bytes.len());
        result
            .bytes
            .extend_from_slice(&buffer[..count.min(remaining)]);
        result.truncated |= count > remaining;
    }
    result
}
