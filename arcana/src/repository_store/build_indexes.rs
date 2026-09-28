use crate::repository::RelationKind;
use crate::synthetic::NodeId;

use super::build::CompactKindIndexRecord;
use super::format::relation_from_code;
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactUnresolvedRecord, RepositoryStoreWriteError,
    StringId,
};

pub(super) fn sorted_dense_ids(
    nodes: &[CompactNodeRecord],
    key: impl Fn(&CompactNodeRecord) -> StringId,
) -> Result<Vec<NodeId>, RepositoryStoreWriteError> {
    let mut ids = Vec::with_capacity(nodes.len());
    for index in 0..nodes.len() {
        ids.push(
            u32::try_from(index)
                .map(NodeId)
                .map_err(|_| RepositoryStoreWriteError::TooManyNodes)?,
        );
    }
    ids.sort_unstable_by_key(|id| (key(&nodes[id.0 as usize]), *id));
    Ok(ids)
}

pub(super) fn sorted_kind_index(
    nodes: &[CompactNodeRecord],
) -> Result<Vec<CompactKindIndexRecord>, RepositoryStoreWriteError> {
    let mut records = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        records.push(CompactKindIndexRecord {
            kind_code: node.kind_code,
            node_id: u32::try_from(index)
                .map(NodeId)
                .map_err(|_| RepositoryStoreWriteError::TooManyNodes)?,
        });
    }
    records.sort_unstable_by_key(|record| (record.kind_code, record.node_id));
    Ok(records)
}

pub(super) fn canonicalize_edges(edges: &mut Vec<CompactEdgeRecord>) {
    edges.sort_unstable_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| relation(left.relation_code).cmp(&relation(right.relation_code)))
            .then_with(|| left.span.cmp(&right.span))
    });
    edges.dedup();
}

pub(super) fn canonicalize_unresolved(records: &mut Vec<CompactUnresolvedRecord>) {
    records.sort_unstable_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| relation(left.relation_code).cmp(&relation(right.relation_code)))
            .then_with(|| left.expression.cmp(&right.expression))
            .then_with(|| {
                optional(left.candidate_namespace).cmp(&optional(right.candidate_namespace))
            })
            .then_with(|| optional(left.candidate_name).cmp(&optional(right.candidate_name)))
            .then_with(|| {
                (left.reason_code, optional(left.unknown_reason))
                    .cmp(&(right.reason_code, optional(right.unknown_reason)))
            })
            .then_with(|| left.span.cmp(&right.span))
    });
    records.dedup();
}

fn relation(code: u16) -> RelationKind {
    relation_from_code(code).expect("compact build only stores valid relation codes")
}

fn optional(id: StringId) -> Option<StringId> {
    id.present()
}

#[cfg(test)]
mod tests {
    use crate::repository::NodeKey;

    use super::super::Sha256Identity;
    use super::super::format::relation_code;
    use super::*;

    #[test]
    fn final_node_indexes_preallocate_from_known_node_count() {
        let nodes = (0..17)
            .map(|index| CompactNodeRecord {
                key: NodeKey::from_u64(index as u64),
                external_identity: Some(Sha256Identity([index as u8; 32])),
                content_id: None,
                path: StringId((16 - index) as u32),
                name: StringId(index as u32),
                qualified_name: StringId(index as u32),
                span: None,
                occurrence_count: 1,
                kind_code: (index % 3) as u16,
            })
            .collect::<Vec<_>>();

        let names = sorted_dense_ids(&nodes, |record| record.name).unwrap();
        let paths = sorted_dense_ids(&nodes, |record| record.path).unwrap();
        let kinds = sorted_kind_index(&nodes).unwrap();

        assert_eq!(names.len(), nodes.len());
        assert_eq!(paths.len(), nodes.len());
        assert_eq!(kinds.len(), nodes.len());
        assert!(names.capacity() >= nodes.len());
        assert!(paths.capacity() >= nodes.len());
        assert!(kinds.capacity() >= nodes.len());
    }

    #[test]
    fn edge_order_uses_relation_semantics_not_store_codes() {
        let source = NodeKey::from_u64(1);
        let target = NodeKey::from_u64(2);
        let mut relations = vec![
            RelationKind::PassesTo,
            RelationKind::Contains,
            RelationKind::Calls,
        ];
        let mut edges = relations
            .iter()
            .rev()
            .map(|relation| CompactEdgeRecord {
                source,
                target,
                relation_code: relation_code(relation),
                span: None,
            })
            .collect::<Vec<_>>();

        canonicalize_edges(&mut edges);
        relations.sort_unstable();

        assert_eq!(
            edges
                .iter()
                .map(|edge| relation(edge.relation_code))
                .collect::<Vec<_>>(),
            relations
        );
    }

    #[test]
    fn lexicon_canonical_sort_deduplicates_duplicate_records() {
        let edge = CompactEdgeRecord {
            source: NodeKey::from_u64(1),
            target: NodeKey::from_u64(2),
            relation_code: relation_code(&RelationKind::Calls),
            span: None,
        };
        let mut edges = vec![edge, edge];
        canonicalize_edges(&mut edges);
        assert_eq!(edges, vec![edge]);

        let unresolved = CompactUnresolvedRecord {
            source: NodeKey::from_u64(1),
            relation_code: relation_code(&RelationKind::Calls),
            reason_code: 1,
            expression: StringId(0),
            candidate_namespace: StringId::ABSENT,
            candidate_name: StringId::ABSENT,
            unknown_reason: StringId::ABSENT,
            span: None,
        };
        let mut records = vec![unresolved, unresolved];
        canonicalize_unresolved(&mut records);
        assert_eq!(records, vec![unresolved]);
    }

    #[test]
    fn unresolved_absent_strings_sort_before_present_ids() {
        let base = CompactUnresolvedRecord {
            source: NodeKey::from_u64(1),
            relation_code: relation_code(&RelationKind::Calls),
            reason_code: 1,
            expression: StringId(0),
            candidate_namespace: StringId(0),
            candidate_name: StringId::ABSENT,
            unknown_reason: StringId::ABSENT,
            span: None,
        };
        let mut records = vec![
            base,
            CompactUnresolvedRecord {
                candidate_namespace: StringId::ABSENT,
                ..base
            },
        ];

        canonicalize_unresolved(&mut records);

        assert_eq!(records[0].candidate_namespace, StringId::ABSENT);
        assert_eq!(records[1].candidate_namespace, StringId(0));
    }
}
