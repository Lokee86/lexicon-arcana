//! Per-search scratch ranks. Fixed memory budget; larger tables spill to disk.
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
const BUDGET: usize = 8 * 1024 * 1024;
const PAGE: usize = 64 * 1024;
pub(super) struct SearchRanks {
    memory: Vec<u8>,
    writer: Option<BufWriter<File>>,
    file: Option<File>,
    pages: VecDeque<(u64, Vec<u8>)>,
    len: u64,
}
impl SearchRanks {
    pub fn new() -> Self {
        Self {
            memory: Vec::new(),
            writer: None,
            file: None,
            pages: VecDeque::new(),
            len: 0,
        }
    }
    pub fn push(&mut self, rank: u8) -> std::io::Result<()> {
        if self.memory.len() == BUDGET && self.writer.is_none() {
            let mut writer = BufWriter::new(tempfile::tempfile()?);
            writer.write_all(&self.memory)?;
            self.memory = Vec::new();
            self.writer = Some(writer);
        }
        if let Some(writer) = &mut self.writer {
            writer.write_all(&[rank])?;
        } else {
            self.memory.push(rank);
        }
        self.len += 1;
        Ok(())
    }
    pub fn finish(&mut self) -> std::io::Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
            self.file = Some(writer.into_inner().map_err(|error| error.into_error())?);
        }
        Ok(())
    }
    pub fn get(&mut self, id: u32) -> std::io::Result<u8> {
        if u64::from(id) >= self.len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "search string ID out of range",
            ));
        }
        let Some(file) = &mut self.file else {
            return Ok(self.memory[id as usize]);
        };
        let base = u64::from(id) / PAGE as u64 * PAGE as u64;
        let index = match self.pages.iter().position(|(key, _)| *key == base) {
            Some(index) => index,
            None => {
                let mut bytes = vec![0; (self.len - base).min(PAGE as u64) as usize];
                file.seek(SeekFrom::Start(base))?;
                file.read_exact(&mut bytes)?;
                if self.pages.len() == BUDGET / PAGE {
                    self.pages.pop_front();
                }
                self.pages.push_back((base, bytes));
                self.pages.len() - 1
            }
        };
        let page = self.pages.remove(index).expect("located scratch page");
        let rank = page.1[(u64::from(id) - base) as usize];
        self.pages.push_back(page);
        Ok(rank)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spills_and_reads_across_pages_in_an_anonymous_file() {
        {
            let mut ranks = SearchRanks::new();
            for i in 0..BUDGET + PAGE + 1 {
                ranks.push((i % 251) as u8).unwrap();
            }
            ranks.finish().unwrap();
            assert!(ranks.file.is_some());
            for i in [0, PAGE - 1, PAGE, BUDGET - 1, BUDGET, BUDGET + PAGE] {
                assert_eq!(ranks.get(i as u32).unwrap(), (i % 251) as u8);
            }
        }
    }
}
