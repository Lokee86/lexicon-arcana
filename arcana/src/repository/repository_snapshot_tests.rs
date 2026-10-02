use std::fs;

use super::repository_snapshot_test_support::{
    request, sample_facts, test_directory, write_artifacts,
};
use super::*;

#[test]
fn binds_graph_and_canonical_repository_store() {
    let directory = test_directory();
    let facts = sample_facts();
    write_artifacts(&directory, &facts);
    publish_repository_snapshot(directory.join(REPOSITORY_MANIFEST_FILE), request()).unwrap();

    let snapshot = RepositorySnapshot::open(directory.join(REPOSITORY_MANIFEST_FILE)).unwrap();
    assert_eq!(snapshot.catalogue().len(), 3);
    assert_eq!(snapshot.graph().edge_count(), 1);
    assert_eq!(snapshot.facts(), &facts);
    assert!(directory.join("repository.arcana").is_file());
    for legacy in ["catalogue.tsv", "unresolved.tsv", "facts.tsv"] {
        assert!(!directory.join(legacy).exists());
    }

    let mut bytes = fs::read(directory.join("repository.arcana")).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(directory.join("repository.arcana"), bytes).unwrap();
    assert!(RepositorySnapshot::open(directory.join(REPOSITORY_MANIFEST_FILE)).is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn manifest_v2_binds_only_graph_and_repository_store() {
    let directory = test_directory();
    let facts = sample_facts();
    write_artifacts(&directory, &facts);
    publish_repository_snapshot(directory.join(REPOSITORY_MANIFEST_FILE), request()).unwrap();

    let text = fs::read_to_string(directory.join(REPOSITORY_MANIFEST_FILE)).unwrap();
    assert!(text.starts_with("version=2\n"));
    assert!(text.contains("repository_store_version=1\n"));
    assert!(text.contains("repository_store_file=repository.arcana\n"));
    for legacy in ["catalogue_file=", "unresolved_file=", "facts_file="] {
        assert!(!text.contains(legacy));
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn precompiled_publication_matches_standalone_without_recompiling() {
    let standalone = test_directory();
    let optimized = test_directory();
    let facts = sample_facts();

    compiler::reset_compile_invocation_count();
    let (compiled, standalone_checksums) = write_artifacts(&standalone, &facts);
    assert_eq!(compiler::compile_invocation_count(), 1);
    crate::storage::write_packed(optimized.join("graph.arcana"), &compiled.dataset).unwrap();
    crate::snapshot::publish_snapshot(optimized.join("graph.manifest"), "graph.arcana", None, 7)
        .unwrap();
    crate::repository_store::write_repository_store(optimized.join("repository.arcana"), &facts)
        .unwrap();
    let optimized_checksums = RepositoryArtifactChecksums {
        repository_store: repository_artifact_file_checksum(optimized.join("repository.arcana"))
            .unwrap(),
    };
    assert_eq!(optimized_checksums, standalone_checksums);

    let optimized_manifest = publish_precompiled_repository_snapshot(
        optimized.join(REPOSITORY_MANIFEST_FILE),
        request(),
        &compiled,
        &facts,
        optimized_checksums,
    )
    .unwrap();
    assert_eq!(compiler::compile_invocation_count(), 1);

    let standalone_manifest =
        publish_repository_snapshot(standalone.join(REPOSITORY_MANIFEST_FILE), request()).unwrap();
    assert_eq!(compiler::compile_invocation_count(), 2);
    assert_eq!(optimized_manifest, standalone_manifest);
    for file in [
        "graph.arcana",
        "graph.manifest",
        "repository.arcana",
        REPOSITORY_MANIFEST_FILE,
    ] {
        assert_eq!(
            fs::read(optimized.join(file)).unwrap(),
            fs::read(standalone.join(file)).unwrap(),
            "{file} differs"
        );
    }

    fs::remove_dir_all(standalone).unwrap();
    fs::remove_dir_all(optimized).unwrap();
}

#[test]
fn update_base_defers_repository_store_until_facts_are_requested() {
    let directory = test_directory();
    let facts = sample_facts();
    let (compiled, checksums) = write_artifacts(&directory, &facts);
    publish_precompiled_repository_snapshot(
        directory.join(REPOSITORY_MANIFEST_FILE),
        request(),
        &compiled,
        &facts,
        checksums,
    )
    .unwrap();

    let base = RepositoryUpdateBase::open(directory.join(REPOSITORY_MANIFEST_FILE)).unwrap();
    fs::remove_file(directory.join("repository.arcana")).unwrap();
    assert!(base.load_facts().is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn update_base_detects_repository_store_checksum_corruption() {
    let directory = test_directory();
    let facts = sample_facts();
    let (compiled, checksums) = write_artifacts(&directory, &facts);
    publish_precompiled_repository_snapshot(
        directory.join(REPOSITORY_MANIFEST_FILE),
        request(),
        &compiled,
        &facts,
        checksums,
    )
    .unwrap();

    let base = RepositoryUpdateBase::open(directory.join(REPOSITORY_MANIFEST_FILE)).unwrap();
    let mut bytes = fs::read(directory.join("repository.arcana")).unwrap();
    bytes[600] ^= 1;
    fs::write(directory.join("repository.arcana"), bytes).unwrap();
    assert!(matches!(
        base.load_facts(),
        Err(RepositorySnapshotError::ArtifactMismatch {
            field: "repository_store_checksum",
            ..
        })
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rich_snapshot_retains_explicit_audit_and_materialization() {
    let directory = test_directory();
    let facts = sample_facts();
    let (compiled, checksums) = write_artifacts(&directory, &facts);
    publish_precompiled_repository_snapshot(
        directory.join(REPOSITORY_MANIFEST_FILE),
        request(),
        &compiled,
        &facts,
        checksums,
    )
    .unwrap();

    let snapshot = RepositorySnapshot::open(directory.join(REPOSITORY_MANIFEST_FILE)).unwrap();
    assert_eq!(snapshot.graph().edge_count(), 1);
    assert_eq!(snapshot.catalogue().len(), 3);
    assert_eq!(snapshot.unresolved().unresolved, compiled.unresolved);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn opening_legacy_v1_manifest_reports_unsupported_version() {
    let directory = test_directory();
    let manifest = [
        "version=1",
        "snapshot_id=94c045dcded9d831",
        "created_unix_seconds=1790472019",
        "repository_id=ac971b0222626ca4",
        "adapter_name=lexicon",
        "adapter_version=sha256:legacy",
        "fact_schema_version=4",
        "node_count=1159295",
        "edge_count=2449939",
        "unresolved_count=724765",
        "graph_snapshot_id=aba89512fdcdf908",
        "graph_manifest_checksum=df67593e78eb3ac4",
        "catalogue_checksum=fd388f0b72fc7ab1",
        "unresolved_checksum=1eba36be551f48fb",
        "facts_checksum=ac971b0222626ca4",
        "graph_manifest_file=graph.manifest",
        "catalogue_file=catalogue.tsv",
        "unresolved_file=unresolved.tsv",
        "facts_file=facts.tsv",
        "",
    ]
    .join("\n");
    let path = directory.join(REPOSITORY_MANIFEST_FILE);
    fs::write(&path, manifest).unwrap();

    assert!(matches!(
        RepositorySnapshot::open(&path),
        Err(RepositorySnapshotError::UnsupportedManifestVersion(1))
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn query_open_uses_no_compiler_and_rejects_inconsistent_or_corrupt_generations() {
    let directory = test_directory();
    let facts = sample_facts();
    write_artifacts(&directory, &facts);
    publish_repository_snapshot(directory.join(REPOSITORY_MANIFEST_FILE), request()).unwrap();
    compiler::reset_compile_invocation_count();
    crate::repository_store::reset_materialization_count();
    let path = directory.join(REPOSITORY_MANIFEST_FILE);
    let query = RepositoryQuerySnapshot::open(&path).unwrap();
    assert_eq!(compiler::compile_invocation_count(), 0);
    assert_eq!(crate::repository_store::materialization_count(), 0);
    assert_eq!(query.len(), 3);
    for id in 0..3 {
        assert_eq!(
            query
                .entry(crate::synthetic::NodeId(id))
                .unwrap()
                .unwrap()
                .node_id
                .0,
            id
        );
    }
    assert_eq!(compiler::compile_invocation_count(), 0);
    drop(query);
    let audit = RepositorySnapshot::open(&path).unwrap();
    assert_eq!(compiler::compile_invocation_count(), 1);
    assert_eq!(crate::repository_store::materialization_count(), 1);
    drop(audit);
    let original = fs::read_to_string(&path).unwrap();
    let mut manifest = RepositorySnapshotManifest::decode(&original).unwrap();
    manifest.unresolved_count += 1;
    fs::write(&path, manifest.encode().unwrap()).unwrap();
    assert!(RepositoryQuerySnapshot::open(&path).is_err());
    fs::write(&path, original).unwrap();
    let mut bytes = fs::read(directory.join("repository.arcana")).unwrap();
    bytes[600] ^= 1;
    fs::write(directory.join("repository.arcana"), bytes).unwrap();
    assert!(RepositoryQuerySnapshot::open(&path).is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn extracted_metadata_reader_keeps_identity_without_reopening_or_materializing() {
    let directory = test_directory();
    write_artifacts(&directory, &sample_facts());
    let path = directory.join(REPOSITORY_MANIFEST_FILE);
    publish_repository_snapshot(&path, request()).unwrap();
    compiler::reset_compile_invocation_count();
    crate::repository_store::reset_materialization_count();
    let query = RepositoryQuerySnapshot::open(&path).unwrap();
    let identity = query.manifest().snapshot_id;
    let mut reader = query.into_metadata_reader();
    assert_eq!(reader.snapshot_id(), identity);
    // Graph and manifest are no longer needed; Windows file handles are released.
    fs::remove_file(directory.join("graph.arcana")).unwrap();
    fs::remove_file(directory.join("graph.manifest")).unwrap();
    fs::remove_file(&path).unwrap();
    for _ in 0..100 {
        for key in [1, 2, 3] {
            assert_eq!(
                reader
                    .lookup_by_key(NodeKey::from_u64(key))
                    .unwrap()
                    .unwrap()
                    .fact
                    .key,
                NodeKey::from_u64(key)
            );
        }
        assert!(reader.cached_bytes() <= 4 * 1024 * 1024);
    }
    assert!(
        reader
            .lookup_by_key(NodeKey::from_u64(99))
            .unwrap()
            .is_none()
    );
    assert_eq!(compiler::compile_invocation_count(), 0);
    assert_eq!(crate::repository_store::materialization_count(), 0);
    drop(reader);
    fs::remove_dir_all(directory).unwrap();
}
