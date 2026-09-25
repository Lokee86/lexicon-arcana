use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::SourceSpan;

use super::model::{AnalysisState, CallEvidence, Declaration};
use super::syntax::identifier_prefix;

static CALL_STATEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bCall\s+([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)").unwrap()
});
static FUNCTION_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\s*\(").unwrap()
});
static NEW_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bNew\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap());

pub fn collect_calls(
    state: &mut AnalysisState,
    span: &SourceSpan,
    text: &str,
    callable: &Declaration,
) {
    let mut seen = BTreeSet::new();
    let mut add = |candidate: &str, expression: &str| {
        let candidate = candidate.trim();
        let key = candidate.to_ascii_lowercase();
        if candidate.is_empty() || reserved(&key) || !seen.insert(key) {
            return;
        }
        state.calls.push(CallEvidence {
            candidate: candidate.into(),
            class_id: callable.class_id.clone(),
            expression: expression.into(),
            owner_id: callable.id.clone(),
            owner_path: callable.owner_path.clone(),
            span: span.clone(),
        });
    };

    for capture in CALL_STATEMENT.captures_iter(text) {
        add(&capture[1], capture.get(0).unwrap().as_str());
    }
    for capture in FUNCTION_CALL.captures_iter(text) {
        let whole = capture.get(0).unwrap();
        let prefix = text[..whole.start()].trim();
        if prefix.to_ascii_lowercase().ends_with("new") {
            continue;
        }
        add(&capture[1], whole.as_str());
    }
    for capture in NEW_CALL.captures_iter(text) {
        add(
            &format!("{}.New", &capture[1]),
            capture.get(0).unwrap().as_str(),
        );
    }
    if let Some(candidate) = bare_call_candidate(text) {
        add(&candidate, text);
    }
}

fn bare_call_candidate(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().next()?.contains(':') {
        return None;
    }
    let candidate = identifier_prefix(trimmed);
    if candidate.is_empty() || reserved(&candidate.to_ascii_lowercase()) {
        return None;
    }
    let remainder = trimmed[candidate.len()..].trim();
    if remainder.is_empty() || remainder.starts_with('=') || remainder.starts_with('.') {
        None
    } else {
        Some(candidate)
    }
}

pub fn builtin(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "array"
            | "cbool"
            | "cbyte"
            | "ccur"
            | "cdate"
            | "cdat"
            | "cdbl"
            | "cint"
            | "clng"
            | "createobject"
            | "csng"
            | "cstr"
            | "chr"
            | "date"
            | "environ"
            | "evaluate"
            | "execute"
            | "format"
            | "getobject"
            | "getthreadinfo"
            | "implode"
            | "instr"
            | "instrrev"
            | "isarray"
            | "iselement"
            | "isempty"
            | "isnull"
            | "isobject"
            | "join"
            | "lbound"
            | "lcase"
            | "left"
            | "len"
            | "lsi_info"
            | "mid"
            | "msgbox"
            | "now"
            | "replace"
            | "right"
            | "run"
            | "shell"
            | "split"
            | "strleft"
            | "strright"
            | "trim"
            | "typename"
            | "ubound"
            | "ucase"
    )
}

fn reserved(name: &str) -> bool {
    matches!(
        name,
        "and"
            | "call"
            | "case"
            | "class"
            | "const"
            | "declare"
            | "dim"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "error"
            | "exit"
            | "for"
            | "forall"
            | "function"
            | "goto"
            | "if"
            | "is"
            | "let"
            | "like"
            | "loop"
            | "mod"
            | "next"
            | "not"
            | "on"
            | "global"
            | "open"
            | "option"
            | "or"
            | "preserve"
            | "print"
            | "private"
            | "property"
            | "protected"
            | "public"
            | "redim"
            | "resume"
            | "select"
            | "set"
            | "static"
            | "stop"
            | "sub"
            | "then"
            | "type"
            | "use"
            | "uselsx"
            | "wend"
            | "while"
            | "with"
            | "xor"
    )
}
