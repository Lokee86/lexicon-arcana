use super::model::{EdgeRecord, FactHeader, FactRecord, NodeRecord, UnresolvedRecord};
use super::order;
use super::validate::{self, ValidationError};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct FactStream {
    pub header: FactHeader,
    pub records: Vec<FactRecord>,
}

impl FactStream {
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        let stream = Self::parse_unvalidated(input)?;
        stream.validate()?;
        Ok(stream)
    }

    pub(crate) fn parse_unvalidated(input: &str) -> Result<Self, ValidationError> {
        let mut lines = input.lines().filter(|line| !line.trim().is_empty());
        let header_line = lines.next().ok_or(ValidationError::EmptyStream)?;
        let header: FactHeader = serde_json::from_str(header_line)
            .map_err(|error| ValidationError::Json(error.to_string()))?;

        let mut records = Vec::new();
        for (offset, line) in lines.enumerate() {
            records.push(parse_record(line, offset + 2)?);
        }

        Ok(Self { header, records })
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate::parts(&self.header, &self.records)
    }

    pub fn canonical_jsonl(&self) -> Result<Vec<u8>, ValidationError> {
        self.validate()?;
        self.jsonl_unchecked()
    }

    pub fn sort_records(&mut self) {
        self.records.sort_by(order::compare);
    }

    pub(crate) fn sort_records_for_export(&mut self) -> Result<(), ValidationError> {
        sort_records(&mut self.records)
    }

    pub(crate) fn jsonl_unchecked(&self) -> Result<Vec<u8>, ValidationError> {
        let mut output = Vec::new();
        write_value(&mut output, &self.header)?;
        for record in &self.records {
            output.push(b'\n');
            let value = tagged_record_value(record)?;
            serde_json::to_writer(&mut output, &value)
                .map_err(|error| ValidationError::Json(error.to_string()))?;
        }
        output.push(b'\n');
        Ok(output)
    }
}

fn parse_record(line: &str, line_number: usize) -> Result<FactRecord, ValidationError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| ValidationError::RecordJson(line_number, error.to_string()))?;
    record_from_value(value, line_number)
}

pub(crate) fn record_from_value(
    value: Value,
    line_number: usize,
) -> Result<FactRecord, ValidationError> {
    let kind = value
        .get("record")
        .and_then(Value::as_str)
        .ok_or(ValidationError::MissingRecordKind(line_number))?;
    match kind {
        "node" => serde_json::from_value::<NodeRecord>(without_record(value))
            .map(FactRecord::Node)
            .map_err(|error| ValidationError::RecordJson(line_number, error.to_string())),
        "edge" => serde_json::from_value::<EdgeRecord>(without_record(value))
            .map(FactRecord::Edge)
            .map_err(|error| ValidationError::RecordJson(line_number, error.to_string())),
        "unresolved" => serde_json::from_value::<UnresolvedRecord>(without_record(value))
            .map(FactRecord::Unresolved)
            .map_err(|error| ValidationError::RecordJson(line_number, error.to_string())),
        other => Err(ValidationError::UnsupportedRecord(other.to_owned())),
    }
}

fn without_record(mut value: Value) -> Value {
    if let Value::Object(ref mut map) = value {
        map.remove("record");
    }
    value
}

fn tagged_record_value(record: &FactRecord) -> Result<Value, ValidationError> {
    match record {
        FactRecord::Node(value) => tagged_value("node", value),
        FactRecord::Edge(value) => tagged_value("edge", value),
        FactRecord::Unresolved(value) => tagged_value("unresolved", value),
    }
}

pub(crate) fn sort_records(records: &mut Vec<FactRecord>) -> Result<(), ValidationError> {
    records.sort_by(order::compare);
    Ok(())
}

fn tagged_value<T: serde::Serialize>(record: &str, value: &T) -> Result<Value, ValidationError> {
    let Value::Object(map) =
        serde_json::to_value(value).map_err(|error| ValidationError::Json(error.to_string()))?
    else {
        return Err(ValidationError::Json("fact record is not an object".into()));
    };
    let mut sorted = BTreeMap::new();
    for (key, value) in map {
        sorted.insert(key, value);
    }
    sorted.insert("record".into(), Value::String(record.into()));
    serde_json::to_value(sorted).map_err(|error| ValidationError::Json(error.to_string()))
}

fn write_value<T: serde::Serialize>(
    output: &mut Vec<u8>,
    value: &T,
) -> Result<(), ValidationError> {
    let value =
        serde_json::to_value(value).map_err(|error| ValidationError::Json(error.to_string()))?;
    serde_json::to_writer(output, &value).map_err(|error| ValidationError::Json(error.to_string()))
}
