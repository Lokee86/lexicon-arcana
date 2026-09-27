mod common;
mod legacy_json;
mod reader;
mod v1;
mod v1_records;
mod v2_fields;
mod v2_read;
mod v2_records;
mod v2_table;
mod v2_write;
mod view;
mod write;

use super::{FactObject, StorageError};
pub(crate) use view::{ObjectView, RecordSelection};

pub(crate) const MAGIC_V1: &[u8; 8] = b"LXOBJ\0\x01\0";
pub(crate) const MAGIC_V2: &[u8; 8] = b"LXOBJ\0\x02\0";
pub(crate) const MAX_STRINGS: u64 = 4_000_000;
pub(crate) const MAX_RECORDS: u64 = 20_000_000;
pub(crate) const MAX_EXTERNAL_REFERENCES: u64 = 4_000_000;
pub(crate) const MAX_STRING_SIZE: u64 = 32 * 1024 * 1024;
pub(crate) const MAX_SECTION_SIZE: u64 = 512 * 1024 * 1024;

pub fn encode_object(object: &FactObject) -> Result<Vec<u8>, StorageError> {
    v2_write::encode(object)
}

pub(crate) fn encode_view(object: &ObjectView<'_>) -> Result<Vec<u8>, StorageError> {
    v2_write::encode_view(object)
}

pub fn decode_object(bytes: &[u8]) -> Result<FactObject, StorageError> {
    let object = if bytes.starts_with(MAGIC_V2) {
        v2_read::decode(bytes)?
    } else if bytes.starts_with(MAGIC_V1) {
        v1::decode(bytes)?
    } else {
        legacy_json::decode(bytes)?
    };
    if object.version != super::OBJECT_VERSION {
        return Err(StorageError::UnsupportedObjectVersion(object.version));
    }
    Ok(object)
}

pub fn decode_node_facts(
    bytes: &[u8],
) -> Result<(FactObject, Vec<crate::NodeRecord>), StorageError> {
    let (object, nodes) = if bytes.starts_with(MAGIC_V2) {
        v2_read::decode_nodes_only(bytes)?
    } else if bytes.starts_with(MAGIC_V1) {
        v1::decode_nodes_only(bytes)?
    } else {
        let object = legacy_json::decode(bytes)?;
        let nodes = object
            .records
            .iter()
            .filter_map(|record| match record {
                crate::FactRecord::Node(node) => Some(node.clone()),
                _ => None,
            })
            .collect();
        (object, nodes)
    };
    if object.version != super::OBJECT_VERSION {
        return Err(StorageError::UnsupportedObjectVersion(object.version));
    }
    Ok((object, nodes))
}

pub(crate) fn is_binary_object(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC_V1) || bytes.starts_with(MAGIC_V2)
}

#[cfg(test)]
mod tests {
    use crate::{FactObject, FactRecord, NodeRecord};

    use super::{ObjectView, RecordSelection, encode_object, encode_view};

    #[test]
    fn borrowed_record_encoding_matches_owned_encoding() {
        let object = FactObject {
            version: 1,
            language: "python".into(),
            owner: "a.py".into(),
            source_content_id: "sha256:source".into(),
            adapter_version: "test".into(),
            schema_version: 1,
            analysis_config_id: "sha256:config".into(),
            records: vec![FactRecord::Node(NodeRecord {
                attributes: None,
                content_id: None,
                id: "node:a".into(),
                kind: "function".into(),
                name: "run".into(),
                owner: Some("a.py".into()),
                path: "a.py".into(),
                qualified_name: "a.run".into(),
                span: None,
            })],
        };
        let refs = object.records.iter().collect::<Vec<_>>();
        let borrowed = ObjectView {
            language: &object.language,
            owner: &object.owner,
            source_content_id: &object.source_content_id,
            adapter_version: &object.adapter_version,
            schema_version: object.schema_version,
            analysis_config_id: &object.analysis_config_id,
            records: RecordSelection::Refs(&refs),
        };

        assert_eq!(
            encode_object(&object).expect("owned encoding"),
            encode_view(&borrowed).expect("borrowed encoding")
        );
    }
}
