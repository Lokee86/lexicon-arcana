use regex::Regex;
use std::sync::LazyLock;

use super::calls::collect_calls;
use super::dataflow::collect_access;
use super::facts::Facts;
use super::model::{AnalysisState, Declaration, SourceFile};
use super::syntax::{contains_word, logical_lines};

static CALLABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^((?:(?:Public|Private|Protected|Static)\s+)*)(Sub|Function|Property\s+(?:Get|Set))\s+([A-Za-z_][A-Za-z0-9_]*)\s*(\([^)]*\))?")
        .unwrap()
});
static CLASS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:(Public|Private|Protected)\s+)?Class\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s+As\s+([A-Za-z_][A-Za-z0-9_.]*))?")
        .unwrap()
});
static EXTERNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^Declare\s+((?:(?:Public|Private)\s+)*)(Sub|Function)\s+([A-Za-z_][A-Za-z0-9_]*)\s*(\([^)]*\))?(.*)$")
        .unwrap()
});
static TYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:(Public|Private)\s+)?Type\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
});
static USE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(UseLSX|Use)\s+(.+)$").unwrap());
static WITH_MEMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[\s(=,+\-*/&<>])\.([A-Za-z_][A-Za-z0-9_]*)").unwrap());

pub fn parse_file(
    state: &mut AnalysisState,
    facts: &mut Facts,
    file: &SourceFile,
    module_id: &str,
) {
    let Ok(source) = std::str::from_utf8(&file.content) else {
        facts.add_unresolved(
            module_id,
            "defines",
            "invalid UTF-8 or NUL-containing source",
            "unsupported-form",
            Some(&file.path),
            None,
            None,
        );
        return;
    };
    if source.as_bytes().contains(&0) {
        facts.add_unresolved(
            module_id,
            "defines",
            "invalid UTF-8 or NUL-containing source",
            "unsupported-form",
            Some(&file.path),
            None,
            None,
        );
        return;
    }

    let mut current_class: Option<Declaration> = None;
    let mut current_callable: Option<Declaration> = None;
    let mut with_stack = Vec::<String>::new();

    for line in logical_lines(&file.path, source) {
        let trimmed = line.text.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower == "option public" {
            state.module_public.insert(file.path.clone(), true);
            continue;
        }
        match lower.as_str() {
            "end sub" | "end function" | "end property" => {
                current_callable = None;
                with_stack.clear();
                continue;
            }
            "end class" | "end type" => {
                current_callable = None;
                current_class = None;
                with_stack.clear();
                continue;
            }
            "end with" => {
                with_stack.pop();
                continue;
            }
            _ => {}
        }

        if let Some(capture) = USE.captures(trimmed) {
            state.add_use(
                facts,
                &file.path,
                module_id,
                &line.span,
                &capture[1],
                &capture[2],
            );
            continue;
        }
        if let Some(capture) = CLASS.captures(trimmed) {
            let declaration = state.add_type(
                facts,
                &file.path,
                module_id,
                &line.span,
                &capture[2],
                capture.get(1).map_or("", |value| value.as_str()),
                capture.get(3).map_or("", |value| value.as_str()),
                false,
            );
            current_class = Some(declaration);
            current_callable = None;
            continue;
        }
        if let Some(capture) = TYPE.captures(trimmed) {
            let declaration = state.add_type(
                facts,
                &file.path,
                module_id,
                &line.span,
                &capture[2],
                capture.get(1).map_or("", |value| value.as_str()),
                "",
                true,
            );
            current_class = Some(declaration);
            current_callable = None;
            continue;
        }
        if let Some(capture) = EXTERNAL.captures(trimmed)
            && contains_word(capture.get(5).map_or("", |value| value.as_str()), "lib")
        {
            state.add_callable(
                facts,
                &file.path,
                module_id,
                &line.span,
                current_class.as_ref(),
                &capture[2],
                &capture[3],
                &capture[1],
                capture.get(4).map_or("", |value| value.as_str()),
                true,
                capture.get(5).map_or("", |value| value.as_str()),
            );
            continue;
        }
        if let Some(capture) = CALLABLE.captures(trimmed) {
            let declaration = state.add_callable(
                facts,
                &file.path,
                module_id,
                &line.span,
                current_class.as_ref(),
                &capture[2],
                &capture[3],
                &capture[1],
                capture.get(4).map_or("", |value| value.as_str()),
                false,
                "",
            );
            current_callable = Some(declaration);
            with_stack.clear();
            continue;
        }

        if let Some(callable) = current_callable.as_ref()
            && lower.starts_with("with ")
        {
            let mut expression = trimmed[4..].trim().to_owned();
            if let Some(receiver) = with_stack.last() {
                expression = expand_with_members(&expression, receiver);
            }
            collect_calls(state, &line.span, &expression, callable);
            collect_access(state, &line.span, &expression, callable);
            with_stack.push(expression);
            continue;
        }

        let call_text = if let Some(receiver) = with_stack.last() {
            expand_with_members(trimmed, receiver)
        } else {
            trimmed.to_owned()
        };
        if state.add_variables(
            facts,
            &file.path,
            module_id,
            &line.span,
            trimmed,
            current_class.as_ref(),
            current_callable.as_ref(),
        ) {
            if let Some(callable) = current_callable.as_ref() {
                collect_calls(state, &line.span, &call_text, callable);
                if lower.starts_with("redim ") {
                    collect_access(state, &line.span, &call_text, callable);
                }
            }
            continue;
        }
        if let Some(callable) = current_callable.as_ref() {
            collect_calls(state, &line.span, &call_text, callable);
            collect_access(state, &line.span, &call_text, callable);
        }
    }
}

fn expand_with_members(text: &str, receiver: &str) -> String {
    if receiver.is_empty() || !text.contains('.') {
        return text.into();
    }
    WITH_MEMBER
        .replace_all(text, |capture: &regex::Captures<'_>| {
            format!("{}{}.{}", &capture[1], receiver, &capture[2])
        })
        .into_owned()
}
