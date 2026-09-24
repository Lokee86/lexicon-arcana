use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::LexiconError;

use super::ConsumerDefinition;

pub const DEFAULT_CONSUMER_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub(super) fn invoke(
    definition: &ConsumerDefinition,
    repository: &Path,
    state_root: &Path,
    snapshot_id: &str,
    output: Option<&mut dyn Write>,
) -> Result<(), LexiconError> {
    let (path, mut capture) = capture_file(state_root)?;
    let stdout = capture.try_clone()?;
    let stderr = capture.try_clone()?;

    let mut command = Command::new(&definition.command);
    command
        .args(&definition.args)
        .current_dir(repository)
        .env("LEXICON_REPOSITORY", repository)
        .env("LEXICON_STATE_ROOT", state_root)
        .env("LEXICON_SNAPSHOT_ID", snapshot_id)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(LexiconError::new(format!(
                "start consumer {}: {error}",
                definition.command
            )));
        }
    };

    let timeout = timeout_for(definition);
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait()?;
            let captured = read_capture(&mut capture)?;
            let _ = std::fs::remove_file(&path);
            return Err(LexiconError::new(format!(
                "context deadline exceeded: {status}: {}",
                String::from_utf8_lossy(&captured).trim()
            )));
        }
        std::thread::sleep(Duration::from_millis(5));
    };

    let captured = read_capture(&mut capture)?;
    let _ = std::fs::remove_file(&path);
    if !status.success() {
        return Err(LexiconError::new(format!(
            "{status}: {}",
            String::from_utf8_lossy(&captured).trim()
        )));
    }

    if let Some(output) = output
        && !captured.is_empty()
    {
        output.write_all(&captured)?;
        if !captured.ends_with(b"\n") {
            output.write_all(b"\n")?;
        }
    }
    Ok(())
}

pub fn timeout_for(definition: &ConsumerDefinition) -> Duration {
    if definition.timeout_nanos == 0 {
        DEFAULT_CONSUMER_TIMEOUT
    } else {
        Duration::from_nanos(definition.timeout_nanos)
    }
}

fn capture_file(state_root: &Path) -> Result<(PathBuf, File), LexiconError> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let directory = state_root.join("tmp");
    std::fs::create_dir_all(&directory)?;
    for _ in 0..32 {
        let path = directory.join(format!(
            "consumer-{}-{}.log",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(LexiconError::new(
        "could not allocate consumer output capture file",
    ))
}

fn read_capture(file: &mut File) -> Result<Vec<u8>, LexiconError> {
    file.flush()?;
    file.seek(SeekFrom::Start(0))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}
