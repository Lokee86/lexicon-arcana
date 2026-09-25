use serde_json::json;

use super::model::SyntaxDiagnostic;

pub fn expression(content: &[u8], diagnostic: &SyntaxDiagnostic) -> String {
    let mut start = diagnostic.token.start_offset.min(content.len());
    let mut end = diagnostic.token.end_offset.clamp(start, content.len());
    while start > 0 && !matches!(content[start - 1], b'\n' | b'\r') {
        start -= 1;
    }
    while end < content.len() && !matches!(content[end], b'\n' | b'\r') {
        end += 1;
    }
    let mut expression = String::from_utf8_lossy(&content[start..end])
        .trim()
        .to_owned();
    if expression.len() > 160 {
        expression.truncate(157);
        expression.push_str("...");
    }
    if expression.is_empty() {
        expression = diagnostic.message.clone();
    }
    expression
}

pub fn attributes(diagnostic: &SyntaxDiagnostic) -> serde_json::Value {
    json!({
        "diagnostic": diagnostic.message,
        "parser": "lexicon-kotlin-structural"
    })
}
