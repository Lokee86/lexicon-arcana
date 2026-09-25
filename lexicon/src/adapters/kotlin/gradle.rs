use crate::SourceSpan;

use super::model::DependencyEvidence;

const CONFIGURATIONS: &[&str] = &[
    "api",
    "compileOnly",
    "implementation",
    "kapt",
    "ksp",
    "runtimeOnly",
    "testImplementation",
];

pub fn parse(path: &str, content: &[u8]) -> Vec<DependencyEvidence> {
    let mut result = Vec::new();
    let mut blocks = Vec::<bool>::new();
    let mut dependencies_depth = 0_i32;
    let mut offset = 0;
    while offset < content.len() {
        let next = skip_trivia(content, offset);
        if next != offset {
            offset = next;
            continue;
        }
        if matches!(content[offset], b'"' | b'\'') {
            offset = skip_string(content, offset);
            continue;
        }
        match content[offset] {
            b'{' => {
                blocks.push(false);
                offset += 1;
                continue;
            }
            b'}' => {
                if let Some(dependencies) = blocks.pop()
                    && dependencies
                {
                    dependencies_depth -= 1;
                }
                offset += 1;
                continue;
            }
            _ => {}
        }
        if !identifier_start(content[offset]) {
            offset += 1;
            continue;
        }
        let start = offset;
        offset = scan_identifier(content, offset);
        let name = std::str::from_utf8(&content[start..offset]).unwrap_or_default();
        if name == "dependencies" {
            let next = skip_spaces_comments(content, offset);
            if next < content.len() && content[next] == b'{' {
                blocks.push(true);
                dependencies_depth += 1;
                offset = next + 1;
            }
            continue;
        }
        if dependencies_depth == 0
            || blocks.last() != Some(&true)
            || !CONFIGURATIONS.contains(&name)
        {
            continue;
        }
        if let Some((dependency, end)) = parse_declaration(path, content, start, offset, name) {
            result.push(dependency);
            offset = end;
        }
    }
    result
}

fn parse_declaration(
    path: &str,
    content: &[u8],
    start: usize,
    after_name: usize,
    configuration: &str,
) -> Option<(DependencyEvidence, usize)> {
    let mut expression_start = after_name;
    while expression_start < content.len() && matches!(content[expression_start], b' ' | b'\t') {
        expression_start += 1;
    }
    if expression_start >= content.len() || matches!(content[expression_start], b'\n' | b'\r') {
        return None;
    }
    let (mut expression_start, mut expression_end, declaration_end) =
        if content[expression_start] == b'(' {
            let close = scan_balanced(content, expression_start).unwrap_or(content.len());
            (
                expression_start + 1,
                close,
                if close < content.len() {
                    close + 1
                } else {
                    close
                },
            )
        } else {
            let end = scan_line_expression(content, expression_start);
            (expression_start, end, end)
        };
    (expression_start, expression_end) = trim_range(content, expression_start, expression_end);
    if expression_start >= expression_end {
        return None;
    }
    let expression =
        String::from_utf8_lossy(&content[expression_start..expression_end]).into_owned();
    let (coordinate, group, artifact, version, resolved) = literal_coordinate(&expression);
    Some((
        DependencyEvidence {
            artifact,
            configuration: configuration.into(),
            coordinate,
            expression,
            group,
            optional: false,
            resolved,
            scope: String::new(),
            span: offset_span(path, content, start, declaration_end),
            version,
        },
        declaration_end,
    ))
}

fn literal_coordinate(expression: &str) -> (String, String, String, String, bool) {
    let value = expression.trim();
    if value.len() < 2 {
        return empty_coordinate();
    }
    let bytes = value.as_bytes();
    if !matches!(bytes[0], b'"' | b'\'') || bytes[value.len() - 1] != bytes[0] {
        return empty_coordinate();
    }
    let coordinate = &value[1..value.len() - 1];
    if coordinate.is_empty()
        || coordinate
            .chars()
            .any(|value| "$\\\"'(){}[] \t\r\n/".contains(value))
    {
        return empty_coordinate();
    }
    let parts = coordinate.split(':').collect::<Vec<_>>();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return empty_coordinate();
    }
    (
        coordinate.into(),
        parts[0].into(),
        parts[1].into(),
        parts.get(2).copied().unwrap_or_default().into(),
        true,
    )
}

fn empty_coordinate() -> (String, String, String, String, bool) {
    Default::default()
}

fn scan_balanced(content: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0_i32;
    let mut offset = open;
    while offset < content.len() {
        let next = skip_trivia(content, offset);
        if next != offset {
            offset = next;
            continue;
        }
        if offset >= content.len() {
            break;
        }
        if matches!(content[offset], b'"' | b'\'') {
            offset = skip_string(content, offset);
            continue;
        }
        match content[offset] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
        offset += 1;
    }
    None
}

fn scan_line_expression(content: &[u8], mut offset: usize) -> usize {
    let mut depth = 0_i32;
    while offset < content.len() {
        if matches!(content[offset], b'"' | b'\'') {
            offset = skip_string(content, offset);
            continue;
        }
        match content[offset] {
            b'(' => depth += 1,
            b')' if depth > 0 => depth -= 1,
            b'\n' | b'\r' | b';' | b'}' if depth == 0 => break,
            b'/' if depth == 0 && content.get(offset + 1) == Some(&b'/') => break,
            _ => {}
        }
        offset += 1;
    }
    offset
}

fn skip_trivia(content: &[u8], mut offset: usize) -> usize {
    if offset >= content.len() {
        return offset;
    }
    if (content[offset] as char).is_whitespace() {
        return offset + 1;
    }
    if content.get(offset..offset + 2) == Some(b"//") {
        while offset < content.len() && content[offset] != b'\n' {
            offset += 1;
        }
        return offset;
    }
    if content.get(offset..offset + 2) == Some(b"/*") {
        if let Some(end) = content[offset + 2..]
            .windows(2)
            .position(|window| window == b"*/")
        {
            return offset + end + 4;
        }
        return content.len();
    }
    offset
}

fn skip_spaces_comments(content: &[u8], mut offset: usize) -> usize {
    while offset < content.len() {
        let next = skip_trivia(content, offset);
        if next == offset {
            break;
        }
        offset = next;
    }
    offset
}

fn skip_string(content: &[u8], mut offset: usize) -> usize {
    let quote = content[offset];
    let triple = quote == b'"'
        && content.get(offset + 1) == Some(&quote)
        && content.get(offset + 2) == Some(&quote);
    if triple {
        offset += 3;
        while offset + 2 < content.len() {
            if content[offset..].starts_with(b"\"\"\"") {
                return offset + 3;
            }
            offset += 1;
        }
        return content.len();
    }
    offset += 1;
    while offset < content.len() {
        if content[offset] == b'\\' {
            offset = (offset + 2).min(content.len());
            continue;
        }
        if content[offset] == quote {
            return offset + 1;
        }
        offset += 1;
    }
    content.len()
}

fn scan_identifier(content: &[u8], mut offset: usize) -> usize {
    while offset < content.len()
        && (identifier_start(content[offset]) || content[offset].is_ascii_digit())
    {
        offset += 1;
    }
    offset
}
fn identifier_start(value: u8) -> bool {
    value == b'_' || value.is_ascii_alphabetic()
}

fn trim_range(content: &[u8], mut start: usize, mut end: usize) -> (usize, usize) {
    while start < end && (content[start] as char).is_whitespace() {
        start += 1;
    }
    while end > start && (content[end - 1] as char).is_whitespace() {
        end -= 1;
    }
    (start, end)
}

pub fn offset_span(path: &str, content: &[u8], start: usize, end: usize) -> SourceSpan {
    let (start_line, start_column) = offset_position(content, start);
    let (end_line, end_column) = offset_position(content, end);
    SourceSpan {
        end_column,
        end_line,
        path: path.into(),
        start_column,
        start_line,
    }
}

fn offset_position(content: &[u8], target: usize) -> (u64, u64) {
    let mut line = 1;
    let mut column = 1;
    let mut offset = 0;
    while offset < target.min(content.len()) {
        if content[offset..].starts_with(b"\r\n") {
            offset += 2;
            line += 1;
            column = 1;
        } else if matches!(content[offset], b'\r' | b'\n') {
            offset += 1;
            line += 1;
            column = 1;
        } else {
            let value = std::str::from_utf8(&content[offset..])
                .ok()
                .and_then(|value| value.chars().next())
                .unwrap_or('\u{FFFD}');
            offset += value.len_utf8().max(1);
            column += 1;
        }
    }
    (line, column)
}
