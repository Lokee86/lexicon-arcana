use std::collections::HashMap;

use crate::repository::{NodeKey, RelationKind};
use crate::synthetic::NodeId;

use super::build::CompactKindIndexRecord;
use super::format::relation_from_code;
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactUnresolvedRecord, RepositoryStoreWriteError,
    StringId,
};

pub(super) fn dense_node_ids(
    nodes: &[CompactNodeRecord],
) -> Result<HashMap<NodeKey, NodeId>, RepositoryStoreWriteError> {
    let mut ids = HashMap::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let id = u32::try_from(index)
            .map(NodeId)
            .map_err(|_| RepositoryStoreWriteError::TooManyNodes)?;
        ids.insert(node.key, id);
    }
    Ok(ids)
}

pub(super) fn sorted_dense_ids(
    nodes: &[CompactNodeRecord],
    key: impl Fn(&CompactNodeRecord) -> StringId,
) -> Result<Vec<NodeId>, RepositoryStoreWriteError> {
    let mut ids = (0..nodes.len())
        .map(|index| {
            u32::try_from(index)
                .map(NodeId)
                .map_err(|_| RepositoryStoreWriteError::TooManyNodes)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort_unstable_by_key(|id| (key(&nodes[id.0 as usize]), *id));
    Ok(ids)
}

pub(super) fn sorted_kind_index(
    nodes: &[CompactNodeRecord],
) -> Result<Vec<CompactKindIndexRecord>, RepositoryStoreWriteError> {
    let mut records = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            Ok(CompactKindIndexRecord {
                kind_code: node.kind_code,
                node_id: u32::try_from(index)
                    .map(NodeId)
                    .map_err(|_| RepositoryStoreWriteError::TooManyNodes)?,
            })
        })
        .collect::<Result<Vec<_>, RepositoryStoreWriteError>>()?;
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
    use super::super::format::relation_code;
    use super::*;

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
