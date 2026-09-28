use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};

use sha2::{Digest, Sha256};

use super::RepositoryStoreWriteError;
use super::format::{ALIGNMENT, HEADER_LEN, RepositoryHeader, SectionSpec};

const WRITE_BUFFER_BYTES: usize = 1024 * 1024;

pub struct PayloadWriter {
    file: BufWriter<File>,
    payload_hasher: Sha256,
    position: u64,
}

impl PayloadWriter {
    pub fn new(mut file: File) -> Result<Self, RepositoryStoreWriteError> {
        file.write_all(&vec![0_u8; usize::from(HEADER_LEN)])?;
        Ok(Self {
            file: BufWriter::with_capacity(WRITE_BUFFER_BYTES, file),
            payload_hasher: Sha256::new(),
            position: u64::from(HEADER_LEN),
        })
    }

    pub fn section<F>(
        &mut self,
        record_count: u64,
        write: F,
    ) -> Result<SectionSpec, RepositoryStoreWriteError>
    where
        F: FnOnce(&mut SectionSink<'_>) -> Result<(), RepositoryStoreWriteError>,
    {
        self.align()?;
        let mut sink = SectionSink {
            file: &mut self.file,
            payload_hasher: &mut self.payload_hasher,
            section_hasher: Sha256::new(),
            position: &mut self.position,
            byte_len: 0,
        };
        write(&mut sink)?;
        Ok(sink.finish(record_count))
    }

    pub fn finish(
        mut self,
        specs: [SectionSpec; super::format::SECTION_COUNT as usize],
    ) -> Result<RepositoryHeader, RepositoryStoreWriteError> {
        self.align()?;
        let payload_checksum: [u8; 32] = self.payload_hasher.finalize().into();
        let header = RepositoryHeader::layout(specs, payload_checksum)?;
        if header.file_len != self.position {
            return Err(RepositoryStoreWriteError::SizeOverflow);
        }
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header.encode())?;
        self.file.flush()?;
        self.file.get_ref().sync_all()?;
        Ok(header)
    }

    fn align(&mut self) -> Result<(), RepositoryStoreWriteError> {
        let aligned = self
            .position
            .checked_add(ALIGNMENT - 1)
            .map(|value| value & !(ALIGNMENT - 1))
            .ok_or(RepositoryStoreWriteError::SizeOverflow)?;
        let padding = usize::try_from(aligned - self.position)
            .map_err(|_| RepositoryStoreWriteError::SizeOverflow)?;
        if padding != 0 {
            let zeros = [0_u8; ALIGNMENT as usize];
            self.file.write_all(&zeros[..padding])?;
            self.payload_hasher.update(&zeros[..padding]);
            self.position = aligned;
        }
        Ok(())
    }
}

pub struct SectionSink<'a> {
    file: &'a mut BufWriter<File>,
    payload_hasher: &'a mut Sha256,
    section_hasher: Sha256,
    position: &'a mut u64,
    byte_len: u64,
}

impl SectionSink<'_> {
    fn finish(self, record_count: u64) -> SectionSpec {
        SectionSpec {
            byte_len: self.byte_len,
            record_count,
            checksum: self.section_hasher.finalize().into(),
        }
    }
}

impl Write for SectionSink<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.file.write_all(bytes)?;
        self.payload_hasher.update(bytes);
        self.section_hasher.update(bytes);
        let len = u64::try_from(bytes.len())
            .map_err(|_| std::io::Error::other("repository.arcana write size overflow"))?;
        *self.position = self
            .position
            .checked_add(len)
            .ok_or_else(|| std::io::Error::other("repository.arcana position overflow"))?;
        self.byte_len = self
            .byte_len
            .checked_add(len)
            .ok_or_else(|| std::io::Error::other("repository.arcana section overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
