use super::{
    ContentId, EdgeFact, FactFileError, NodeFact, NodeKey, NodeKind, RelationKind, RepositoryFacts,
    SourceSpan, UnresolvedReason, UnresolvedReferenceFact, normalize_repository_path,
};

#[path = "lexicon_fact_file.rs"]
mod lexicon_fact_file;

const HEADER_V1: &str = "version\t1";
const HEADER_V2: &str = "version\t2";
const HEADER_V3: &str = "version\t3";
pub const FACT_SCHEMA_VERSION: u64 = 4;
const HEADER_V4: &str = "version\t4";

/// Parses the canonical tab-separated repository fact format.
pub fn parse_facts(input: &str) -> Result<RepositoryFacts, FactFileError> {
    if input.trim_start().starts_with('{') {
        return lexicon_fact_file::parse_lexicon_facts(input);
    }
    let mut lines = input.lines();
    let version = parse_header(lines.next().ok_or(FactFileError::InvalidHeader)?)?;

    let mut facts = RepositoryFacts::default();
    for (index, line) in lines.enumerate() {
        parse_record_line(line, index + 2, version, &mut facts)?;
    }
    Ok(facts)
}

pub(super) fn parse_header(line: &str) -> Result<u64, FactFileError> {
    match line {
        HEADER_V1 => Ok(1),
        HEADER_V2 => Ok(2),
        HEADER_V3 => Ok(3),
        HEADER_V4 => Ok(4),
        _ => Err(FactFileError::InvalidHeader),
    }
}

pub(super) fn parse_record_line(
    line: &str,
    line_number: usize,
    version: u64,
    facts: &mut RepositoryFacts,
) -> Result<(), FactFileError> {
    if line.is_empty() {
        return Err(FactFileError::MalformedLine { line: line_number });
    }
    let fields = line
        .split('\t')
        .map(|field| unescape(field, line_number))
        .collect::<Result<Vec<_>, _>>()?;
    match fields.first().map(String::as_str) {
        Some("N") => facts.nodes.push(parse_node(&fields, line_number, version)?),
        Some("E") => facts.edges.push(parse_edge(&fields, line_number)?),
        Some("U") if version >= 2 => facts
            .unresolved
            .push(parse_unresolved(&fields, line_number)?),
        _ => return Err(FactFileError::UnknownRecord { line: line_number }),
    }
    Ok(())
}

fn parse_node(fields: &[String], line: usize, version: u64) -> Result<NodeFact, FactFileError> {
    let (
        identity_index,
        kind_index,
        path_index,
        name_index,
        qualified_name_index,
        content_index,
        span_start,
    ) = if version >= 4 {
        if fields.len() != 13 {
            return Err(FactFileError::MalformedLine { line });
        }
        (Some(2), 3, 4, 5, Some(6), 7, 8)
    } else if version >= 3 {
        if fields.len() != 12 {
            return Err(FactFileError::MalformedLine { line });
        }
        (Some(2), 3, 4, 5, None, 6, 7)
    } else {
        if fields.len() != 11 {
            return Err(FactFileError::MalformedLine { line });
        }
        (None, 2, 3, 4, None, 5, 6)
    };
    let path = normalize_repository_path(&fields[path_index])
        .map_err(|_| FactFileError::MalformedLine { line })?;
    let external_identity = identity_index
        .map(|index| parse_external_identity(&fields[index], line))
        .transpose()?
        .flatten();
    Ok(NodeFact {
        key: parse_id(&fields[1], line).map(NodeKey::from_u64)?,
        external_identity,
        kind: NodeKind::parse(&fields[kind_index]).ok_or(FactFileError::InvalidKind { line })?,
        path,
        name: fields[name_index].clone(),
        qualified_name: qualified_name_index
            .map(|index| fields[index].clone())
            .unwrap_or_else(|| fields[name_index].clone()),
        content_id: parse_optional_id(&fields[content_index], line)?.map(ContentId::from_u64),
        span: parse_span(&fields[span_start..span_start + 5], line)?,
    })
}

fn parse_external_identity(value: &str, line: usize) -> Result<Option<String>, FactFileError> {
    if value == "-" {
        return Ok(None);
    }
    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(FactFileError::InvalidNumber { line });
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(FactFileError::InvalidNumber { line });
    }
    Ok(Some(value.to_owned()))
}

fn parse_edge(fields: &[String], line: usize) -> Result<EdgeFact, FactFileError> {
    if fields.len() != 9 {
        return Err(FactFileError::MalformedLine { line });
    }
    Ok(EdgeFact {
        source: NodeKey::from_u64(parse_id(&fields[1], line)?),
        target: NodeKey::from_u64(parse_id(&fields[2], line)?),
        relation: RelationKind::parse(&fields[3]).ok_or(FactFileError::InvalidRelation { line })?,
        span: parse_span(&fields[4..9], line)?,
    })
}

fn parse_unresolved(
    fields: &[String],
    line: usize,
) -> Result<UnresolvedReferenceFact, FactFileError> {
    if fields.len() != 12 {
        return Err(FactFileError::MalformedLine { line });
    }
    Ok(UnresolvedReferenceFact {
        source: NodeKey::from_u64(parse_id(&fields[1], line)?),
        relation: RelationKind::parse(&fields[2]).ok_or(FactFileError::InvalidRelation { line })?,
        reason: UnresolvedReason::parse(&fields[3]).ok_or(FactFileError::InvalidReason { line })?,
        expression: fields[4].clone(),
        candidate_namespace: parse_optional_field(&fields[5]),
        candidate_name: parse_optional_field(&fields[6]),
        span: parse_span(&fields[7..12], line)?,
    })
}

fn parse_span(fields: &[String], line: usize) -> Result<Option<SourceSpan>, FactFileError> {
    if fields.len() != 5 {
        return Err(FactFileError::InvalidSpan { line });
    }
    let absent = fields.iter().all(|field| field == "-");
    if absent {
        return Ok(None);
    }
    if fields.iter().any(|field| field == "-") {
        return Err(FactFileError::InvalidSpan { line });
    }
    let path =
        normalize_repository_path(&fields[0]).map_err(|_| FactFileError::InvalidSpan { line })?;
    Ok(Some(SourceSpan {
        path,
        start_line: parse_u32(&fields[1], line)?,
        start_column: parse_u32(&fields[2], line)?,
        end_line: parse_u32(&fields[3], line)?,
        end_column: parse_u32(&fields[4], line)?,
    }))
}

fn parse_optional_id(value: &str, line: usize) -> Result<Option<u64>, FactFileError> {
    if value == "-" {
        Ok(None)
    } else {
        parse_id(value, line).map(Some)
    }
}

fn parse_id(value: &str, line: usize) -> Result<u64, FactFileError> {
    if value.len() != 16 {
        return Err(FactFileError::InvalidNumber { line });
    }
    u64::from_str_radix(value, 16).map_err(|_| FactFileError::InvalidNumber { line })
}

fn parse_u32(value: &str, line: usize) -> Result<u32, FactFileError> {
    value
        .parse()
        .map_err(|_| FactFileError::InvalidNumber { line })
}

fn parse_optional_field(value: &str) -> Option<String> {
    (value != "-").then(|| value.to_owned())
}

fn unescape(value: &str, line: usize) -> Result<String, FactFileError> {
    let mut output = String::with_capacity(value.len());
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            output.push(match character {
                '\\' => '\\',
                't' => '\t',
                'n' => '\n',
                'r' => '\r',
                '0' => '\0',
                _ => return Err(FactFileError::InvalidEscape { line }),
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    if escaped {
        return Err(FactFileError::InvalidEscape { line });
    }
    Ok(output)
}
