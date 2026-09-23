use super::common::{NODE_KINDS, RELATIONS, attributes_bytes};
use super::v2_table::{References, Table, build};
use super::write;
use super::{MAGIC_V2, MAX_EXTERNAL_REFERENCES};
use crate::{FactObject, FactRecord, StorageError};

pub(crate) fn encode(object: &FactObject) -> Result<Vec<u8>, StorageError> {
    validate_metadata(object)?;
    let (table, references) = build(object);
    if references.external.len() as u64 > MAX_EXTERNAL_REFERENCES {
        return Err(StorageError::Binary(
            "external reference count exceeds limit".into(),
        ));
    }

    let mut output = Vec::new();
    output.extend_from_slice(MAGIC_V2);
    write::uvarint(&mut output, super::super::OBJECT_VERSION);
    write::uvarint(&mut output, object.schema_version);
    write_string_table(&mut output, &table);
    write::string_ref(&mut output, &table.index, &object.language);
    write::string_ref(&mut output, &table.index, &object.owner);
    write::identity(&mut output, &table.index, &object.source_content_id);
    write::string_ref(&mut output, &table.index, &object.adapter_version);
    write::identity(&mut output, &table.index, &object.analysis_config_id);
    write::uvarint(&mut output, references.external.len() as u64);
    for value in &references.external {
        write::identity(&mut output, &table.index, value);
    }

    write::section(&mut output, &nodes(object, &table)?);
    write::section(&mut output, &edges(object, &table, &references)?);
    write::section(&mut output, &unresolved(object, &table, &references)?);
    Ok(output)
}

fn validate_metadata(object: &FactObject) -> Result<(), StorageError> {
    if object.language.is_empty() {
        return Err(StorageError::InvalidObject("language"));
    }
    if object.adapter_version.is_empty() {
        return Err(StorageError::InvalidObject("adapter_version"));
    }
    if object.schema_version == 0 {
        return Err(StorageError::InvalidObject("schema_version"));
    }
    if object.analysis_config_id.is_empty() {
        return Err(StorageError::InvalidObject("analysis_config_id"));
    }
    Ok(())
}

fn write_string_table(output: &mut Vec<u8>, table: &Table) {
    write::uvarint(output, table.values.len() as u64);
    for (index, value) in table.values.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .map_or("", |previous| table.values[previous].as_str());
        let prefix = common_prefix(previous.as_bytes(), value.as_bytes());
        write::uvarint(output, prefix as u64);
        write::bytes(output, &value.as_bytes()[prefix..]);
    }
}

fn nodes(object: &FactObject, table: &Table) -> Result<Vec<u8>, StorageError> {
    let records: Vec<_> = object
        .records
        .iter()
        .filter(|record| matches!(record, FactRecord::Node(_)))
        .collect();
    let mut output = Vec::new();
    write::uvarint(&mut output, records.len() as u64);
    for record in records {
        let FactRecord::Node(node) = record else {
            unreachable!()
        };
        write::bytes(&mut output, &attributes_bytes(record)?);
        write::identity(
            &mut output,
            &table.index,
            node.content_id.as_deref().unwrap_or(""),
        );
        write::identity(&mut output, &table.index, &node.id);
        write::code_or_string(&mut output, &table.index, &node.kind, NODE_KINDS);
        write::string_ref(&mut output, &table.index, &node.name);
        write::factored(
            &mut output,
            &table.index,
            node.owner.as_deref().unwrap_or(""),
            &object.owner,
        );
        write::factored(&mut output, &table.index, &node.path, &object.owner);
        write::qname(
            &mut output,
            &table.index,
            &node.qualified_name,
            &node.name,
            &node.path,
            &object.owner,
        );
        write::span(&mut output, &table.index, node.span.as_ref());
    }
    Ok(output)
}

fn edges(
    object: &FactObject,
    table: &Table,
    references: &References,
) -> Result<Vec<u8>, StorageError> {
    let records: Vec<_> = object
        .records
        .iter()
        .filter(|record| matches!(record, FactRecord::Edge(_)))
        .collect();
    let mut output = Vec::new();
    write::uvarint(&mut output, records.len() as u64);
    for record in records {
        let FactRecord::Edge(edge) = record else {
            unreachable!()
        };
        write::bytes(&mut output, &attributes_bytes(record)?);
        write::factored(
            &mut output,
            &table.index,
            edge.owner.as_deref().unwrap_or(""),
            &object.owner,
        );
        write::code_or_string(&mut output, &table.index, &edge.relation, RELATIONS);
        node_ref(&mut output, references, &edge.source);
        write::span(&mut output, &table.index, edge.span.as_ref());
        node_ref(&mut output, references, &edge.target);
    }
    Ok(output)
}

fn unresolved(
    object: &FactObject,
    table: &Table,
    references: &References,
) -> Result<Vec<u8>, StorageError> {
    let records: Vec<_> = object
        .records
        .iter()
        .filter(|record| matches!(record, FactRecord::Unresolved(_)))
        .collect();
    let mut output = Vec::new();
    write::uvarint(&mut output, records.len() as u64);
    for record in records {
        let FactRecord::Unresolved(value) = record else {
            unreachable!()
        };
        write::bytes(&mut output, &attributes_bytes(record)?);
        for text in [
            value.candidate_name.as_deref().unwrap_or(""),
            value.candidate_namespace.as_deref().unwrap_or(""),
            &value.expression,
        ] {
            write::string_ref(&mut output, &table.index, text);
        }
        write::factored(
            &mut output,
            &table.index,
            value.owner.as_deref().unwrap_or(""),
            &object.owner,
        );
        write::string_ref(&mut output, &table.index, &value.reason);
        write::code_or_string(&mut output, &table.index, &value.relation, RELATIONS);
        node_ref(&mut output, references, &value.source);
        write::span(&mut output, &table.index, value.span.as_ref());
    }
    Ok(output)
}

fn node_ref(output: &mut Vec<u8>, references: &References, value: &str) {
    if let Some(index) = references.local.get(value) {
        output.push(0);
        write::uvarint(output, index + 1);
    } else {
        output.push(1);
        write::uvarint(output, references.external_index[value] + 1);
    }
}

fn common_prefix(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}
