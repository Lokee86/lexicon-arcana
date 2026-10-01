use crate::repository::NodeKey;

use super::Sha256Identity;
use super::build_stream::TempNodeRecord;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StagedNodeError {
    IdentityCollision { key: NodeKey },
    ConflictingDefinition { identity: Sha256Identity },
}

pub(super) fn canonicalize_nodes(nodes: &mut Vec<TempNodeRecord>) -> Result<(), StagedNodeError> {
    nodes.sort_unstable_by_key(|record| record.key);
    if nodes.len() < 2 {
        return Ok(());
    }

    let mut write = 1_usize;
    for read in 1..nodes.len() {
        let current = nodes[read];
        let previous = nodes[write - 1];

        if current.key != previous.key {
            nodes[write] = current;
            write += 1;
            continue;
        }
        if current.external_identity != previous.external_identity {
            return Err(StagedNodeError::IdentityCollision { key: current.key });
        }
        if current.signature_digest != previous.signature_digest {
            return Err(StagedNodeError::ConflictingDefinition {
                identity: current.external_identity,
            });
        }
    }
    nodes.truncate(write);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::ContentId;
    use crate::repository_store::{TempSpan, TempStringId};

    fn node(key: NodeKey, identity: u8, signature: u8, name: u32) -> TempNodeRecord {
        TempNodeRecord {
            key,
            external_identity: Sha256Identity([identity; 32]),
            signature_digest: [signature; 32],
            content_id: Some(ContentId::from_bytes(&[identity])),
            owner: Some(TempStringId(1)),
            path: TempStringId(2),
            name: TempStringId(name),
            qualified_name: TempStringId(4),
            span: Some(TempSpan {
                path: TempStringId(2),
                start_line: 1,
                start_column: 2,
                end_line: 3,
                end_column: 4,
            }),
            kind_code: 5,
        }
    }

    #[test]
    fn sorts_by_key_and_collapses_exact_duplicates() {
        let low = NodeKey::from_u64(1);
        let high = NodeKey::from_u64(2);
        let duplicate = node(high, 9, 7, 3);
        let mut nodes = vec![duplicate, node(low, 8, 6, 2), duplicate];

        canonicalize_nodes(&mut nodes).unwrap();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].key, low);
        assert_eq!(nodes[1].key, high);
        assert_eq!(nodes[1], duplicate);
    }

    #[test]
    fn rejects_same_identity_with_conflicting_semantics() {
        let key = NodeKey::from_u64(1);
        let mut nodes = vec![node(key, 9, 7, 3), node(key, 9, 8, 4)];

        assert_eq!(
            canonicalize_nodes(&mut nodes),
            Err(StagedNodeError::ConflictingDefinition {
                identity: Sha256Identity([9; 32]),
            })
        );
    }

    #[test]
    fn rejects_distinct_identities_with_the_same_node_key() {
        let key = NodeKey::from_u64(1);
        let mut nodes = vec![node(key, 9, 7, 3), node(key, 8, 7, 3)];

        assert_eq!(
            canonicalize_nodes(&mut nodes),
            Err(StagedNodeError::IdentityCollision { key })
        );
    }
}
