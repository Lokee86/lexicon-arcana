use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::repository::{
    ContentId, EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind, RepositoryFacts, SourceSpan,
    UnresolvedReason, UnresolvedReferenceFact,
};

use super::format::{
    EDGE_RECORD_LEN, HEADER_LEN, NODE_RECORD_LEN, RepositoryHeader, SectionKind,
    UNRESOLVED_RECORD_LEN,
};
use super::{CompactEdgeRecord, CompactNodeRecord, CompactStringTable, CompactUnresolvedRecord};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

pub fn sample_facts() -> RepositoryFacts {
    let file = node(2, NodeKind::File, "src/a.rs", "a.rs", "crate::a", None);
    let mut function = node(
        3,
        NodeKind::Function,
        "src/a.rs",
        "odd\tname\n☃",
        "crate::a::odd\tname\n☃",
        Some(span("src/a.rs", 7, 1, 9, 4)),
    );
    function.external_identity = Some(format!("sha256:{}", "ab".repeat(32)));
    function.content_id = Some(ContentId::from_u64(0x44));
    let other = node(4, NodeKind::File, "src/b.rs", "b.rs", "crate::b", None);

    let call = EdgeFact {
        source: function.key,
        target: other.key,
        relation: RelationKind::Calls,
        span: Some(span("src/a.rs", 8, 3, 8, 11)),
    };
    let reference = EdgeFact {
        source: function.key,
        target: file.key,
        relation: RelationKind::References,
        span: None,
    };
    let unresolved = UnresolvedReferenceFact {
        source: function.key,
        relation: RelationKind::PossibleCalls,
        expression: "callee\twith\ncontrols\\\0☃".into(),
        candidate_namespace: Some("-".into()),
        candidate_name: None,
        reason: UnresolvedReason::Unknown("future\treason".into()),
        span: Some(span("src/a.rs", 9, 2, 9, 14)),
    };

    RepositoryFacts::with_unresolved(
        vec![
            node(1, NodeKind::Repository, "repo", "repo", "repo", None),
            file,
            function.clone(),
            function,
            other,
        ],
        vec![call.clone(), reference, call],
        vec![unresolved.clone(), unresolved],
    )
}

pub fn decode_facts(bytes: &[u8]) -> RepositoryFacts {
    let header = RepositoryHeader::decode(&bytes[..usize::from(HEADER_LEN)]).unwrap();
    let strings_section = section(bytes, &header, SectionKind::Strings);
    let strings = CompactStringTable::decode(
        strings_section,
        header.section(SectionKind::Strings).record_count,
    )
    .unwrap();

    let mut facts = RepositoryFacts::default();
    for chunk in section(bytes, &header, SectionKind::Nodes).chunks_exact(NODE_RECORD_LEN as usize)
    {
        let record = CompactNodeRecord::decode(chunk.try_into().unwrap()).unwrap();
        let fact = record.to_fact(&strings).unwrap();
        for _ in 0..record.occurrence_count {
            facts.nodes.push(fact.clone());
        }
    }
    for chunk in section(bytes, &header, SectionKind::Edges).chunks_exact(EDGE_RECORD_LEN as usize)
    {
        let record = CompactEdgeRecord::decode(chunk.try_into().unwrap()).unwrap();
        facts.edges.push(record.to_fact(&strings).unwrap());
    }
    for chunk in section(bytes, &header, SectionKind::Unresolved)
        .chunks_exact(UNRESOLVED_RECORD_LEN as usize)
    {
        let record = CompactUnresolvedRecord::decode(chunk.try_into().unwrap()).unwrap();
        facts.unresolved.push(record.to_fact(&strings).unwrap());
    }
    facts
}

pub fn verify_checksums(bytes: &[u8]) {
    let header = RepositoryHeader::decode(&bytes[..usize::from(HEADER_LEN)]).unwrap();
    let payload: [u8; 32] = Sha256::digest(&bytes[usize::from(HEADER_LEN)..]).into();
    assert_eq!(header.payload_checksum, payload);
    for kind in SectionKind::ALL {
        let digest: [u8; 32] = Sha256::digest(section(bytes, &header, kind)).into();
        assert_eq!(header.section(kind).checksum, digest);
    }
}

pub fn temp_path(label: &str) -> PathBuf {
    let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "arcana-repository-store-{}-{id}-{label}.arcana",
        std::process::id()
    ))
}

pub fn cleanup(paths: &[&PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

pub fn section<'a>(bytes: &'a [u8], header: &RepositoryHeader, kind: SectionKind) -> &'a [u8] {
    let descriptor = header.section(kind);
    let start = descriptor.offset as usize;
    let end = start + descriptor.byte_len as usize;
    &bytes[start..end]
}

fn node(
    key: u64,
    kind: NodeKind,
    path: &str,
    name: &str,
    qualified_name: &str,
    span: Option<SourceSpan>,
) -> NodeFact {
    NodeFact {
        key: NodeKey::from_u64(key),
        external_identity: None,
        kind,
        path: path.into(),
        name: name.into(),
        qualified_name: qualified_name.into(),
        content_id: None,
        span,
    }
}

fn span(
    path: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> SourceSpan {
    SourceSpan {
        path: path.into(),
        start_line,
        start_column,
        end_line,
        end_column,
    }
}
