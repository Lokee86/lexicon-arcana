use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::format::SectionKind;
use super::{RepositoryStoreFile, RepositoryStoreWriteError};

static REWRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct RewriteWorkspace {
    root: PathBuf,
}

impl RewriteWorkspace {
    pub(super) fn new(output: &Path) -> Result<Self, RepositoryStoreWriteError> {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        let sequence = REWRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(".repository-rewrite-{}-{sequence}", std::process::id());
        let root = parent.join(name);
        std::fs::create_dir(&root)?;
        Ok(Self { root })
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for RewriteWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(super) struct FixedSectionReader<const N: usize> {
    reader: BufReader<File>,
    index: u64,
    count: u64,
}

impl<const N: usize> FixedSectionReader<N> {
    pub(super) fn open(
        store: &RepositoryStoreFile,
        kind: SectionKind,
    ) -> Result<Self, RepositoryStoreWriteError> {
        let section = store.header.section(kind);
        let mut file = File::open(store.path())?;
        file.seek(SeekFrom::Start(section.offset))?;
        Ok(Self {
            reader: BufReader::new(file),
            index: 0,
            count: section.record_count,
        })
    }

    pub(super) fn next(&mut self) -> Result<Option<(u64, [u8; N])>, RepositoryStoreWriteError> {
        if self.index == self.count {
            return Ok(None);
        }
        let index = self.index;
        let mut bytes = [0_u8; N];
        self.reader.read_exact(&mut bytes)?;
        self.index += 1;
        Ok(Some((index, bytes)))
    }
}

pub(super) struct FixedMapFile {
    file: File,
    width: u64,
}

impl FixedMapFile {
    pub(super) fn create(
        path: &Path,
        count: u64,
        width: u64,
    ) -> Result<Self, RepositoryStoreWriteError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        file.set_len(
            count
                .checked_mul(width)
                .ok_or(RepositoryStoreWriteError::SizeOverflow)?,
        )?;
        Ok(Self { file, width })
    }

    pub(super) fn set_u32(
        &mut self,
        index: u64,
        value: u32,
    ) -> Result<(), RepositoryStoreWriteError> {
        debug_assert_eq!(self.width, 4);
        self.write_at(index, &value.to_le_bytes())
    }

    pub(super) fn get_u32(&mut self, index: u64) -> Result<u32, RepositoryStoreWriteError> {
        debug_assert_eq!(self.width, 4);
        let mut bytes = [0_u8; 4];
        self.read_at(index, &mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) fn set_u64(
        &mut self,
        index: u64,
        value: u64,
    ) -> Result<(), RepositoryStoreWriteError> {
        debug_assert_eq!(self.width, 8);
        self.write_at(index, &value.to_le_bytes())
    }

    pub(super) fn get_u64(&mut self, index: u64) -> Result<u64, RepositoryStoreWriteError> {
        debug_assert_eq!(self.width, 8);
        let mut bytes = [0_u8; 8];
        self.read_at(index, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn write_at(&mut self, index: u64, bytes: &[u8]) -> Result<(), RepositoryStoreWriteError> {
        let offset = index
            .checked_mul(self.width)
            .ok_or(RepositoryStoreWriteError::SizeOverflow)?;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(bytes)?;
        Ok(())
    }

    fn read_at(&mut self, index: u64, bytes: &mut [u8]) -> Result<(), RepositoryStoreWriteError> {
        let offset = index
            .checked_mul(self.width)
            .ok_or(RepositoryStoreWriteError::SizeOverflow)?;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(bytes)?;
        Ok(())
    }
}

pub(super) fn copy_file(
    path: &Path,
    sink: &mut impl Write,
) -> Result<(), RepositoryStoreWriteError> {
    let mut reader = BufReader::new(File::open(path)?);
    std::io::copy(&mut reader, sink)?;
    Ok(())
}
