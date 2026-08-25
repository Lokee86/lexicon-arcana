use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Edge;
use crate::storage::{PackedGraph, StableHasher};

use super::overlay_format::{HEADER_LEN, OverlayHeader, OverlayLayout, operation_checksum};
use super::overlay_validation::validate_changes;
use super::{OverlayChanges, OverlayError, OverlayWriteSummary};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn write_overlay(
    path: impl AsRef<Path>,
    base: &PackedGraph,
    changes: &OverlayChanges,
) -> Result<OverlayWriteSummary, OverlayError> {
    let path = path.as_ref();
    if path.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("overlay already exists: {}", path.display()),
        )
        .into());
    }
    let temp_path = temporary_path(path);
    let result = write_then_commit(path, &temp_path, base, changes);
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_then_commit(
    path: &Path,
    temp_path: &Path,
    base: &PackedGraph,
    changes: &OverlayChanges,
) -> Result<OverlayWriteSummary, OverlayError> {
    let validated = validate_changes(base, changes)?;
    let added_count = validated.added.len() as u64;
    let removed_count = validated.removed.len() as u64;
    let layout = OverlayLayout::for_counts(added_count, removed_count)?;
    let overlay_checksum = operation_checksum(
        base.node_count(),
        base.edge_count(),
        base.dataset_checksum(),
        &validated.added,
        &validated.removed,
    );

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&[0_u8; HEADER_LEN as usize])?;
    let mut payload = PayloadWriter::new(&mut writer, u64::from(HEADER_LEN));
    payload.pad_to(layout.added_edges)?;
    write_edges(&mut payload, &validated.added)?;
    payload.pad_to(layout.removed_edges)?;
    write_edges(&mut payload, &validated.removed)?;
    payload.pad_to(layout.file_len)?;
    let payload_checksum = payload.finish();

    let header = OverlayHeader {
        node_count: base.node_count(),
        base_edge_count: base.edge_count(),
        added_count,
        removed_count,
        base_dataset_checksum: base.dataset_checksum(),
        visible_edge_count: validated.visible_edge_count,
        visible_dataset_checksum: validated.visible_dataset_checksum,
        overlay_checksum,
        payload_checksum,
        layout,
    };
    writer.seek(SeekFrom::Start(0))?;
    writer.write_all(&header.encode())?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);
    fs::rename(temp_path, path)?;

    Ok(OverlayWriteSummary {
        node_count: base.node_count(),
        base_edge_count: base.edge_count(),
        added_count,
        removed_count,
        visible_edge_count: validated.visible_edge_count,
        base_dataset_checksum: base.dataset_checksum(),
        visible_dataset_checksum: validated.visible_dataset_checksum,
        overlay_checksum,
        file_len: layout.file_len,
    })
}

fn write_edges(writer: &mut PayloadWriter<'_>, edges: &[Edge]) -> Result<(), OverlayError> {
    for edge in edges {
        writer.write(&edge.source.0.to_le_bytes())?;
        writer.write(&edge.target.0.to_le_bytes())?;
        writer.write(&edge.kind.0.to_le_bytes())?;
    }
    Ok(())
}

struct PayloadWriter<'a> {
    writer: &'a mut BufWriter<File>,
    hasher: StableHasher,
    position: u64,
}

impl<'a> PayloadWriter<'a> {
    fn new(writer: &'a mut BufWriter<File>, position: u64) -> Self {
        Self {
            writer,
            hasher: StableHasher::new(),
            position,
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), OverlayError> {
        self.writer.write_all(bytes)?;
        self.hasher.update(bytes);
        self.position = self
            .position
            .checked_add(bytes.len() as u64)
            .ok_or(OverlayError::SizeOverflow)?;
        Ok(())
    }

    fn pad_to(&mut self, target: u64) -> Result<(), OverlayError> {
        if self.position > target {
            return Err(OverlayError::SizeOverflow);
        }
        let zeros = [0_u8; 8];
        while self.position < target {
            let count = usize::try_from((target - self.position).min(8))
                .map_err(|_| OverlayError::SizeOverflow)?;
            self.write(&zeros[..count])?;
        }
        Ok(())
    }

    fn finish(self) -> u64 {
        self.hasher.finish()
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("arcana-overlay"), OsString::from);
    name.push(format!(".tmp.{}.{}", std::process::id(), sequence));
    path.with_file_name(name)
}
