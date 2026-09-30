use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use super::RepositoryStoreWriteError;

const CHUNK_VALUES: usize = 262_144;
const MERGE_FAN_IN: usize = 32;

pub(super) fn sort_unique_u32(
    input: &Path,
    output: &Path,
    work: &Path,
) -> Result<(), RepositoryStoreWriteError> {
    let mut runs = make_runs(input, work)?;
    if runs.is_empty() {
        File::create(output)?;
        return Ok(());
    }

    let mut pass = 0_u32;
    while runs.len() > 1 {
        let mut next = Vec::new();
        for (group, chunk) in runs.chunks(MERGE_FAN_IN).enumerate() {
            let path = work.join(format!("u32-merge-{pass}-{group}.bin"));
            merge_runs(chunk, &path)?;
            next.push(path);
        }
        for path in runs {
            let _ = std::fs::remove_file(path);
        }
        runs = next;
        pass += 1;
    }

    std::fs::rename(runs.pop().expect("non-empty runs"), output)?;
    Ok(())
}

fn make_runs(input: &Path, work: &Path) -> Result<Vec<PathBuf>, RepositoryStoreWriteError> {
    let mut reader = BufReader::new(File::open(input)?);
    let mut runs = Vec::new();
    let mut chunk = Vec::with_capacity(CHUNK_VALUES);

    loop {
        chunk.clear();
        while chunk.len() < CHUNK_VALUES {
            match read_u32(&mut reader)? {
                Some(value) => chunk.push(value),
                None => break,
            }
        }
        if chunk.is_empty() {
            break;
        }
        chunk.sort_unstable();
        chunk.dedup();

        let path = work.join(format!("u32-run-{}.bin", runs.len()));
        let mut writer = BufWriter::new(File::create(&path)?);
        for value in &chunk {
            writer.write_all(&value.to_le_bytes())?;
        }
        writer.flush()?;
        runs.push(path);

        if chunk.len() < CHUNK_VALUES {
            break;
        }
    }
    Ok(runs)
}

fn merge_runs(inputs: &[PathBuf], output: &Path) -> Result<(), RepositoryStoreWriteError> {
    let mut readers = inputs
        .iter()
        .map(File::open)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(BufReader::new)
        .collect::<Vec<_>>();
    let mut heap = BinaryHeap::<Reverse<(u32, usize)>>::new();

    for (index, reader) in readers.iter_mut().enumerate() {
        if let Some(value) = read_u32(reader)? {
            heap.push(Reverse((value, index)));
        }
    }

    let mut writer = BufWriter::new(File::create(output)?);
    let mut previous = None;
    while let Some(Reverse((value, reader_index))) = heap.pop() {
        if previous != Some(value) {
            writer.write_all(&value.to_le_bytes())?;
            previous = Some(value);
        }
        if let Some(next) = read_u32(&mut readers[reader_index])? {
            heap.push(Reverse((next, reader_index)));
        }
    }
    writer.flush()?;
    Ok(())
}

pub(super) fn read_u32(reader: &mut impl Read) -> Result<Option<u32>, RepositoryStoreWriteError> {
    let mut bytes = [0_u8; 4];
    match reader.read_exact(&mut bytes) {
        Ok(()) => Ok(Some(u32::from_le_bytes(bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(error) => Err(error.into()),
    }
}
