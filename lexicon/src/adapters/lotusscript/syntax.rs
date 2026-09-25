use crate::SourceSpan;

use super::model::LogicalLine;

pub fn logical_lines(path: &str, source: &str) -> Vec<LogicalLine> {
    let normalized = source.replace("\r\n", "\n");
    let physical = normalized.split('\n').collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut parts = Vec::new();
    let mut start_line = 0_usize;
    let mut in_block_comment = false;

    for (index, raw) in physical.iter().enumerate() {
        let line_number = index + 1;
        let trimmed = raw.trim().trim_end_matches('\r');
        if in_block_comment {
            if is_block_comment_end(trimmed) {
                in_block_comment = false;
            }
            continue;
        }
        if is_block_comment_start(trimmed) {
            in_block_comment = true;
            continue;
        }

        let clean = strip_comment(trimmed).trim().to_owned();
        if start_line == 0 && clean.is_empty() {
            continue;
        }
        if start_line == 0 {
            start_line = line_number;
        }
        let continued = clean.ends_with('_');
        let clean = if continued {
            clean.trim_end_matches('_').trim().to_owned()
        } else {
            clean
        };
        if !clean.is_empty() {
            parts.push(clean);
        }
        if continued {
            continue;
        }

        let text = parts.join(" ").trim().to_owned();
        if !text.is_empty() {
            let span = SourceSpan {
                end_column: raw.chars().count() as u64 + 1,
                end_line: line_number as u64,
                path: path.into(),
                start_column: 1,
                start_line: start_line as u64,
            };
            result.extend(split_statements(span, &text));
        }
        parts.clear();
        start_line = 0;
    }
    result
}

pub fn split_top_level(value: &str, separator: char) -> Vec<String> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    let mut quote = false;
    let mut pipe = false;
    let mut date = false;
    let chars = value.char_indices().collect::<Vec<_>>();

    let mut cursor = 0;
    while cursor < chars.len() {
        let (index, current) = chars[cursor];
        match current {
            '"' if !pipe => {
                if quote
                    && chars
                        .get(cursor + 1)
                        .is_some_and(|(_, value)| *value == '"')
                {
                    cursor += 1;
                } else {
                    quote = !quote;
                }
            }
            '|' if !quote && !date => pipe = !pipe,
            '#' if !quote && !pipe => date = !date,
            '(' if !quote && !pipe && !date => depth += 1,
            ')' if !quote && !pipe && !date && depth > 0 => depth -= 1,
            _ if current == separator && depth == 0 && !quote && !pipe && !date => {
                result.push(value[start..index].trim().to_owned());
                start = index + current.len_utf8();
            }
            _ => {}
        }
        cursor += 1;
    }
    result.push(value[start..].trim().to_owned());
    result
}

pub fn literal_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() < 2 {
        return None;
    }
    let bytes = value.as_bytes();
    if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
        || (bytes[0] == b'|' && bytes[value.len() - 1] == b'|')
    {
        Some(value[1..value.len() - 1].to_owned())
    } else {
        None
    }
}

pub fn identifier_prefix(value: &str) -> String {
    let value = value.trim();
    let mut chars = value.char_indices();
    let Some((_, first)) = chars.next() else {
        return String::new();
    };
    if first != '_' && !first.is_alphabetic() {
        return String::new();
    }
    let mut end = first.len_utf8();
    for (index, current) in chars {
        if current != '_' && !current.is_alphanumeric() {
            break;
        }
        end = index + current.len_utf8();
    }
    value[..end].to_owned()
}

pub fn contains_word(value: &str, word: &str) -> bool {
    value
        .split(|current: char| {
            !(current == '_' || current == '-' || current == '.' || current.is_ascii_alphanumeric())
        })
        .any(|current| current.eq_ignore_ascii_case(word))
}

pub fn mask_literals(value: &str) -> String {
    let mut bytes = value.as_bytes().to_vec();
    let mut quote = false;
    let mut pipe = false;
    let mut date = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                if pipe || date {
                    bytes[index] = b' ';
                } else if quote && bytes.get(index + 1) == Some(&b'"') {
                    bytes[index] = b' ';
                    bytes[index + 1] = b' ';
                    index += 1;
                } else {
                    quote = !quote;
                    bytes[index] = b' ';
                }
            }
            b'|' => {
                if !quote && !date {
                    pipe = !pipe;
                }
                bytes[index] = b' ';
            }
            b'#' => {
                if !quote && !pipe {
                    date = !date;
                }
                bytes[index] = b' ';
            }
            _ if quote || pipe || date => bytes[index] = b' ',
            _ => {}
        }
        index += 1;
    }
    String::from_utf8(bytes).unwrap_or_default()
}

fn split_statements(span: SourceSpan, text: &str) -> Vec<LogicalLine> {
    let mut parts = split_top_level(text, ':');
    if parts.len() > 1 {
        let first = parts.first().map_or("", String::as_str).trim();
        if !first.is_empty() && identifier_prefix(first) == first {
            parts.remove(0);
        }
    }
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .map(|text| LogicalLine {
            span: span.clone(),
            text,
        })
        .collect()
}

fn strip_comment(line: &str) -> &str {
    let trimmed = line.trim();
    if trimmed.len() >= 3
        && trimmed[..3].eq_ignore_ascii_case("rem")
        && (trimmed.len() == 3 || trimmed[3..].chars().next().is_some_and(char::is_whitespace))
    {
        return "";
    }
    let bytes = line.as_bytes();
    let mut quote = false;
    let mut pipe = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if !pipe => {
                if quote && bytes.get(index + 1) == Some(&b'"') {
                    index += 1;
                } else {
                    quote = !quote;
                }
            }
            b'|' if !quote => pipe = !pipe,
            b'\'' if !quote && !pipe => return &line[..index],
            _ => {}
        }
        index += 1;
    }
    line
}

fn is_block_comment_start(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("%REM")
}

fn is_block_comment_end(line: &str) -> bool {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    fields.len() == 2
        && fields[0].eq_ignore_ascii_case("%END")
        && fields[1].eq_ignore_ascii_case("REM")
}
