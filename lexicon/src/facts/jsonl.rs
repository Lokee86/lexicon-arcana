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
        let mut lines = input.lines().filter(|line| !line.trim().is_empty());
        let header_line = lines.next().ok_or(ValidationError::EmptyStream)?;
        let header: FactHeader = serde_json::from_str(header_line)
            .map_err(|error| ValidationError::Json(error.to_string()))?;

        let mut records = Vec::new();
        for (offset, line) in lines.enumerate() {
            records.push(parse_record(line, offset + 2)?);
        }

        let stream = Self { header, records };
        stream.validate()?;
        Ok(stream)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate::stream(self)
    }

    pub fn canonical_jsonl(&self) -> Result<Vec<u8>, ValidationError> {
        self.validate()?;
        let mut output = Vec::new();
        write_value(&mut output, &self.header)?;
        for record in &self.records {
            output.push(b'\n');
            let value = match record {
                FactRecord::Node(value) => tagged_value("node", value)?,
                FactRecord::Edge(value) => tagged_value("edge", value)?,
                FactRecord::Unresolved(value) => tagged_value("unresolved", value)?,
            };
            serde_json::to_writer(&mut output, &value)
                .map_err(|error| ValidationError::Json(error.to_string()))?;
        }
        output.push(b'\n');
        Ok(output)
    }

    pub fn sort_records(&mut self) {
        self.records.sort_by(order::compare);
    }
}

fn parse_record(line: &str, line_number: usize) -> Result<FactRecord, ValidationError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| ValidationError::RecordJson(line_number, error.to_string()))?;
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
