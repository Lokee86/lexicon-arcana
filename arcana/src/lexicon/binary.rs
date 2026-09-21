use super::LexiconSnapshotError;
use super::object::{EdgeRecord, FactObject, FactRecord, NodeRecord, SpanRecord, UnresolvedRecord};

const MAGIC_V1: &[u8; 8] = b"LXOBJ\0\x01\0";
const MAX_STRINGS: u64 = 4_000_000;
const MAX_RECORDS: u64 = 20_000_000;
const MAX_STRING_SIZE: u64 = 32 * 1024 * 1024;
const MAX_SECTION_SIZE: u64 = 512 * 1024 * 1024;

pub(super) fn is_binary_object(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC_V1) || bytes.starts_with(super::binary_v2::MAGIC)
}

pub(super) fn parse_binary_object(bytes: &[u8]) -> Result<FactObject, LexiconSnapshotError> {
    if bytes.starts_with(super::binary_v2::MAGIC) {
        return super::binary_v2::parse_binary_object(bytes);
    }
    let mut reader = Reader::new(bytes);
    reader.expect_magic()?;
    let version = reader.uvarint("object version")?;
    let schema_version = reader.uvarint("schema version")?;
    let strings = reader.string_table()?;
    let language = reader.string_ref(&strings, "language")?.to_owned();
    let owner = optional(reader.string_ref(&strings, "owner")?);
    let source_content_id = optional(reader.string_ref(&strings, "source content ID")?);
    let adapter_version = reader.string_ref(&strings, "adapter version")?.to_owned();
    let analysis_config_id = reader
        .string_ref(&strings, "analysis config ID")?
        .to_owned();
    let nodes = reader.bytes("node section", MAX_SECTION_SIZE)?;
    let edges = reader.bytes("edge section", MAX_SECTION_SIZE)?;
    let unresolved = reader.bytes("unresolved section", MAX_SECTION_SIZE)?;
    reader.finish("fact object")?;

    let mut records = decode_nodes(nodes, &strings)?;
    records.extend(decode_edges(edges, &strings)?);
    records.extend(decode_unresolved(unresolved, &strings)?);
    Ok(FactObject {
        version,
        language,
        owner,
        source_content_id,
        adapter_version,
        schema_version,
        analysis_config_id,
        records,
    })
}

fn decode_nodes(bytes: &[u8], strings: &[String]) -> Result<Vec<FactRecord>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("node records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Node(NodeRecord {
            attributes: reader.attributes()?,
            content_id: optional(reader.string_ref(strings, "node content ID")?),
            id: reader.string_ref(strings, "node ID")?.to_owned(),
            kind: reader.string_ref(strings, "node kind")?.to_owned(),
            name: reader.string_ref(strings, "node name")?.to_owned(),
            owner: optional(reader.string_ref(strings, "node owner")?),
            path: reader.string_ref(strings, "node path")?.to_owned(),
            qualified_name: reader
                .string_ref(strings, "node qualified name")?
                .to_owned(),
            span: reader.span(strings)?,
        }));
    }
    reader.finish("node section")?;
    Ok(records)
}

fn decode_edges(bytes: &[u8], strings: &[String]) -> Result<Vec<FactRecord>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("edge records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: reader.attributes()?,
            owner: optional(reader.string_ref(strings, "edge owner")?),
            relation: reader.string_ref(strings, "edge relation")?.to_owned(),
            source: reader.string_ref(strings, "edge source")?.to_owned(),
            span: reader.span(strings)?,
            target: reader.string_ref(strings, "edge target")?.to_owned(),
        }));
    }
    reader.finish("edge section")?;
    Ok(records)
}

fn decode_unresolved(
    bytes: &[u8],
    strings: &[String],
) -> Result<Vec<FactRecord>, LexiconSnapshotError> {
    let mut reader = Reader::new(bytes);
    let count = reader.count("unresolved records", MAX_RECORDS)?;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(FactRecord::Unresolved(UnresolvedRecord {
            attributes: reader.attributes()?,
            candidate_name: optional(reader.string_ref(strings, "candidate name")?),
            candidate_namespace: optional(reader.string_ref(strings, "candidate namespace")?),
            expression: reader.string_ref(strings, "expression")?.to_owned(),
            owner: optional(reader.string_ref(strings, "unresolved owner")?),
            reason: reader.string_ref(strings, "unresolved reason")?.to_owned(),
            relation: reader
                .string_ref(strings, "unresolved relation")?
                .to_owned(),
            source: reader.string_ref(strings, "unresolved source")?.to_owned(),
            span: reader.span(strings)?,
        }));
    }
    reader.finish("unresolved section")?;
    Ok(records)
}

fn optional(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn expect_magic(&mut self) -> Result<(), LexiconSnapshotError> {
        if !self.bytes.starts_with(MAGIC_V1) {
            return Err(binary_error("invalid object magic"));
        }
        self.position = MAGIC_V1.len();
        Ok(())
    }

    fn uvarint(&mut self, field: &str) -> Result<u64, LexiconSnapshotError> {
        let mut value = 0_u64;
        for shift in (0..70).step_by(7) {
            let byte = self.byte(field)?;
            if shift == 63 && byte > 1 {
                return Err(binary_error(format!("invalid {field} varint")));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(binary_error(format!("invalid {field} varint")))
    }

    fn count(&mut self, field: &str, maximum: u64) -> Result<usize, LexiconSnapshotError> {
        let value = self.uvarint(field)?;
        if value > maximum {
            return Err(binary_error(format!("{field} count exceeds limit")));
        }
        usize::try_from(value).map_err(|_| binary_error(format!("{field} count overflows")))
    }

    fn byte(&mut self, field: &str) -> Result<u8, LexiconSnapshotError> {
        let value = self
            .bytes
            .get(self.position)
            .copied()
            .ok_or_else(|| binary_error(format!("truncated before {field}")))?;
        self.position += 1;
        Ok(value)
    }

    fn bytes(&mut self, field: &str, maximum: u64) -> Result<&'a [u8], LexiconSnapshotError> {
        let length = self.uvarint(&format!("{field} length"))?;
        if length > maximum {
            return Err(binary_error(format!("{field} length exceeds limit")));
        }
        let length = usize::try_from(length)
            .map_err(|_| binary_error(format!("{field} length overflows")))?;
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| binary_error(format!("{field} length overflows")))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| binary_error(format!("truncated in {field}")))?;
        self.position = end;
        Ok(value)
    }

    fn string_table(&mut self) -> Result<Vec<String>, LexiconSnapshotError> {
        let count = self.count("string table", MAX_STRINGS)?;
        if count == 0 {
            return Err(binary_error("missing empty string sentinel"));
        }
        let mut strings = Vec::with_capacity(count);
        for _ in 0..count {
            let bytes = self.bytes("string", MAX_STRING_SIZE)?;
            strings.push(
                std::str::from_utf8(bytes)
                    .map_err(|_| binary_error("string table contains invalid UTF-8"))?
                    .to_owned(),
            );
        }
        if !strings[0].is_empty() {
            return Err(binary_error("invalid empty string sentinel"));
        }
        Ok(strings)
    }

    fn string_ref<'b>(
        &mut self,
        strings: &'b [String],
        field: &str,
    ) -> Result<&'b str, LexiconSnapshotError> {
        let index = self.uvarint(field)?;
        let index = usize::try_from(index)
            .map_err(|_| binary_error(format!("{field} string index overflows")))?;
        strings
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| binary_error(format!("{field} string index is out of range")))
    }

    fn attributes(&mut self) -> Result<Option<Vec<u8>>, LexiconSnapshotError> {
        let value = self.bytes("record attributes", MAX_STRING_SIZE)?;
        Ok((!value.is_empty()).then(|| value.to_vec()))
    }

    fn span(&mut self, strings: &[String]) -> Result<Option<SpanRecord>, LexiconSnapshotError> {
        match self.byte("span flag")? {
            0 => Ok(None),
            1 => Ok(Some(SpanRecord {
                path: self.string_ref(strings, "span path")?.to_owned(),
                start_line: self.uvarint("span start line")?,
                start_column: self.uvarint("span start column")?,
                end_line: self.uvarint("span end line")?,
                end_column: self.uvarint("span end column")?,
            })),
            _ => Err(binary_error("invalid span flag")),
        }
    }

    fn finish(&self, field: &str) -> Result<(), LexiconSnapshotError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(binary_error(format!("{field} has trailing bytes")))
        }
    }
}

fn binary_error(message: impl Into<String>) -> LexiconSnapshotError {
    LexiconSnapshotError::Binary(message.into())
}
