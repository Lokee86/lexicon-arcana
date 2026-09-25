use super::{
    model::MacroCallExpression,
    syntax::{normalize_qualified, normalize_space},
};

const NON_CALL_TARGETS: &[&str] = &[
    "_Alignof",
    "_Generic",
    "_Static_assert",
    "alignof",
    "defined",
    "do",
    "else",
    "for",
    "if",
    "return",
    "sizeof",
    "static_assert",
    "switch",
    "typeof",
    "typeof_unqual",
    "while",
];

pub fn semantic_details(
    definition: &str,
    name: &str,
    function_like: bool,
) -> (String, Vec<String>, Vec<MacroCallExpression>) {
    let Some(index) = definition.find(name) else {
        return (String::new(), Vec::new(), Vec::new());
    };
    let mut remainder = &definition[index + name.len()..];
    let mut parameters = Vec::new();
    if function_like {
        remainder = remainder.trim_start_matches([' ', '\t']);
        if remainder.starts_with('(')
            && let Some(end) = matching_delimiter(remainder, 0, b'(', b')')
        {
            parameters = parameter_names(&remainder[1..end]);
            remainder = &remainder[end + 1..];
        }
    }
    let replacement = normalize_space(&remainder.replace("\\\r\n", " ").replace("\\\n", " "));
    let calls = call_expressions(&replacement);
    (replacement, parameters, calls)
}

pub fn direct_target(replacement: &str) -> String {
    let mut value = replacement.trim();
    while let Some(rest) = value.strip_prefix('(') {
        value = rest.trim_start();
    }
    let (candidate, end) = read_callee(value, 0);
    if candidate.is_empty() || NON_CALL_TARGETS.contains(&candidate.as_str()) {
        return String::new();
    }
    let next = skip_space(value, end);
    if next == value.len() || value.as_bytes().get(next) == Some(&b'(') {
        candidate
    } else {
        String::new()
    }
}

pub fn split_arguments(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        return Vec::new();
    }
    let bytes = value.as_bytes();
    let (mut start, mut round, mut square, mut curly) = (0usize, 0isize, 0isize, 0isize);
    let mut result = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => index = skip_quoted(value, index),
            b'(' => round += 1,
            b')' => round -= 1,
            b'[' => square += 1,
            b']' => square -= 1,
            b'{' => curly += 1,
            b'}' => curly -= 1,
            b',' if round == 0 && square == 0 && curly == 0 => {
                result.push(value[start..index].trim().to_owned());
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    result.push(value[start..].trim().to_owned());
    result
}

pub fn matching_delimiter(value: &str, open: usize, left: u8, right: u8) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut depth = 0isize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => index = skip_quoted(value, index),
            byte if byte == left => depth += 1,
            byte if byte == right => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

pub fn skip_quoted(value: &str, start: usize) -> usize {
    let bytes = value.as_bytes();
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        if bytes[index] == quote {
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

pub fn identifier_start(value: u8) -> bool {
    value == b'_' || value.is_ascii_alphabetic()
}

pub fn identifier_part(value: u8) -> bool {
    identifier_start(value) || value.is_ascii_digit()
}

fn parameter_names(value: &str) -> Vec<String> {
    split_arguments(value)
        .into_iter()
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                None
            } else if part == "..." {
                Some("__VA_ARGS__".into())
            } else {
                Some(part.trim_end_matches("...").trim().to_owned())
            }
        })
        .collect()
}

fn call_expressions(replacement: &str) -> Vec<MacroCallExpression> {
    let bytes = replacement.as_bytes();
    let mut calls = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if matches!(bytes[index], b'"' | b'\'') {
            index = skip_quoted(replacement, index);
            continue;
        }
        if !identifier_start(bytes[index]) {
            index += 1;
            continue;
        }
        let start = index;
        let (callee, end) = read_callee(replacement, index);
        index = end;
        if callee.is_empty() || NON_CALL_TARGETS.contains(&callee.as_str()) {
            continue;
        }
        let open = skip_space(replacement, end);
        if replacement.as_bytes().get(open) != Some(&b'(') {
            continue;
        }
        let Some(close) = matching_delimiter(replacement, open, b'(', b')') else {
            continue;
        };
        if start > 0 && matches!(bytes[start - 1], b'.' | b'>') {
            continue;
        }
        let expression = &replacement[start..=close];
        let token_pasting = expression.contains("##");
        let stringification = expression.replace("##", "").contains('#');
        let variadic_substitution = expression.contains("__VA_ARGS__")
            || expression.contains("__VA_OPT__")
            || expression.contains("...");
        calls.push(MacroCallExpression {
            callee: normalize_qualified(&callee),
            arguments: split_arguments(&replacement[open + 1..close]),
            token_pasting,
            stringification,
            variadic_substitution,
            unsupported: token_pasting || stringification || variadic_substitution,
        });
    }
    calls
}

fn read_callee(value: &str, start: usize) -> (String, usize) {
    let bytes = value.as_bytes();
    if start >= bytes.len() || !identifier_start(bytes[start]) {
        return (String::new(), start);
    }
    let mut index = start + 1;
    while index < bytes.len() && identifier_part(bytes[index]) {
        index += 1;
    }
    loop {
        let space = skip_space(value, index);
        if bytes.get(space) != Some(&b':') || bytes.get(space + 1) != Some(&b':') {
            return (value[start..index].to_owned(), index);
        }
        index = skip_space(value, space + 2);
        if index >= bytes.len() || !identifier_start(bytes[index]) {
            return (value[start..index].to_owned(), index);
        }
        index += 1;
        while index < bytes.len() && identifier_part(bytes[index]) {
            index += 1;
        }
    }
}

fn skip_space(value: &str, mut index: usize) -> usize {
    let bytes = value.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::{direct_target, semantic_details};

    #[test]
    fn parses_parameters_calls_and_unsupported_substitutions() {
        let (replacement, parameters, calls) = semantic_details(
            "#define APPLY(array, index) push(array[index], index)",
            "APPLY",
            true,
        );
        assert_eq!(replacement, "push(array[index], index)");
        assert_eq!(parameters, ["array", "index"]);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].callee, "push");
        assert_eq!(calls[0].arguments, ["array[index]", "index"]);
        assert_eq!(direct_target(&replacement), "push");

        let (_, parameters, calls) = semantic_details(
            "#define LOG(format, ...) log(format, __VA_ARGS__)",
            "LOG",
            true,
        );
        assert_eq!(parameters, ["format", "__VA_ARGS__"]);
        assert!(calls[0].unsupported);
        assert!(calls[0].variadic_substitution);
    }
}
