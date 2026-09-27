use std::hint::black_box;

use super::{RepositoryStore, RepositoryStoreFile};

fn inputs() -> (String, Vec<String>) {
    let store = std::env::var("ARCANA_REPOSITORY_STORE_BENCH")
        .expect("ARCANA_REPOSITORY_STORE_BENCH must name repository.arcana");
    let paths = std::env::var("ARCANA_REPOSITORY_PATH_BENCH")
        .unwrap_or_else(|_| "apps/desktop/electron/oauth-net-request.ts".to_owned())
        .split(';')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();
    (store, paths)
}

#[test]
#[ignore = "manual large-store full-buffer reader benchmark"]
fn benchmark_full_reader_large_store() {
    let (store, paths) = inputs();
    let store = RepositoryStore::open(store).unwrap();
    let keys = store.owned_node_keys(&paths).unwrap();
    eprintln!(
        "full-reader file_len={} keys={}",
        store.header().file_len,
        keys.len()
    );
    black_box((store, keys));
}

#[test]
#[ignore = "manual large-store file-backed reader benchmark"]
fn benchmark_file_reader_large_store() {
    let (store, paths) = inputs();
    let mut store = RepositoryStoreFile::open(store).unwrap();
    let keys = store.owned_node_keys(&paths).unwrap();
    eprintln!(
        "file-reader file_len={} keys={} checksum={:016x}",
        store.header().file_len,
        keys.len(),
        store.artifact_checksum()
    );
    black_box((store, keys));
}
