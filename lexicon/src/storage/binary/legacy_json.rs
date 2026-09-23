use crate::facts::record_from_value;
use crate::{FactObject, StorageError};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct WireObject {
    version: u64,
    language: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    source_content_id: String,
    adapter_version: String,
    schema_version: u64,
    analysis_config_id: String,
    #[serde(default)]
    records: Vec<Value>,
}

pub(crate) fn decode(bytes: &[u8]) -> Result<FactObject, StorageError> {
    let wire: WireObject = serde_json::from_slice(trim_ascii(bytes))?;
    let records = wire
        .records
        .into_iter()
        .enumerate()
        .map(|(index, value)| record_from_value(value, index + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FactObject {
        version: wire.version,
        language: wire.language,
        owner: wire.owner,
        source_content_id: wire.source_content_id,
        adapter_version: wire.adapter_version,
        schema_version: wire.schema_version,
        analysis_config_id: wire.analysis_config_id,
        records,
    })
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    &bytes[start..end]
}
