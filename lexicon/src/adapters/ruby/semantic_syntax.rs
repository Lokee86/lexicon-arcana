fn namespace(frames: &[Frame]) -> String {
    frames
        .iter()
        .filter_map(|f| match f {
            Frame::Namespace(n) => Some(n.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("::")
}

fn namespace_open(line: &str, _frames: &[Frame]) -> Option<String> {
    let rest = line
        .strip_prefix("class ")
        .or_else(|| line.strip_prefix("module "))?;
    if rest.starts_with("<<") {
        return None;
    }
    let raw = rest.split(['<', ';']).next()?.trim();
    if raw.is_empty() {
        return None;
    }
    Some(raw.to_string())
}

fn method_open(line: &str) -> Option<(String, Vec<String>, bool)> {
    let rest = line.strip_prefix("def ")?;
    let head = rest.split(';').next()?.trim();
    let raw_name = head.split(['(', ' ']).next()?.trim();
    let singleton = raw_name.starts_with("self.");
    let name = raw_name.trim_start_matches("self.").to_string();
    let params = head
        .split_once('(')
        .and_then(|(_, tail)| tail.split_once(')'))
        .map(|(inside, _)| {
            inside
                .split(',')
                .map(|p| p.trim().trim_start_matches('*').to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some((name, params, singleton))
}

fn inside_singleton_class(frames: &[Frame]) -> bool {
    matches!(frames.last(), Some(Frame::Other))
}

fn include_target(line: &str) -> Option<String> {
    let rest = line.strip_prefix("include ")?;
    Some(
        rest.split_whitespace()
            .next()?
            .trim_end_matches(',')
            .to_string(),
    )
}

fn explicit_include(line: &str) -> Option<(String, String)> {
    let (host, rest) = line.split_once(".include(")?;
    Some((host.trim().into(), rest.trim_end_matches(')').trim().into()))
}

fn struct_assignment(line: &str) -> Option<(String, Vec<String>)> {
    let (name, rhs) = line.split_once('=')?;
    if !rhs.contains("Struct.new(") && !rhs.contains("Data.define(") {
        return None;
    }
    let open = rhs.find('(')?;
    let close = rhs[open + 1..].find(')')? + open + 1;
    let accessors = rhs[open + 1..close]
        .split(',')
        .map(|s| {
            s.trim()
                .trim_start_matches(':')
                .trim_matches(['"', '\''])
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    Some((name.trim().to_string(), accessors))
}

fn constructor_type(text: &str) -> Option<String> {
    for part in text.split_whitespace() {
        if let Some(pos) = part.find(".new") {
            let candidate = part[..pos].trim_matches(|c: char| !c.is_alphanumeric() && c != ':');
            if candidate.chars().next().is_some_and(|c| c.is_uppercase()) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn assignment(line: &str) -> Option<(String, String, bool)> {
    for op in ["+=", "-=", "*=", "/=", "="] {
        if let Some((lhs, rhs)) = line.split_once(op) {
            let lhs = lhs.trim();
            if lhs.contains(' ') || lhs.is_empty() || lhs == "==" {
                continue;
            }
            return Some((lhs.into(), rhs.trim().into(), op != "="));
        }
    }
    None
}

fn identifiers(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '@'))
        .filter(|s| !s.is_empty() && !s.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

fn call_expressions(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("def ")
        || trimmed.starts_with("class ")
        || trimmed.starts_with("module ")
    {
        return out;
    }

    for raw in trimmed.split(';') {
        let mut expr = raw.trim();
        if let Some((_, rhs, _)) = assignment(expr) {
            expr = Box::leak(rhs.into_boxed_str());
        }
        if let Some(rest) = expr.strip_prefix("return ") {
            expr = rest;
        }
        if expr == "super" || expr.starts_with("super(") {
            out.push("super".into());
            continue;
        }
        if expr.contains("send(") {
            out.push(expr.into());
            continue;
        }

        for token in expr.split_whitespace() {
            let token = token.trim_matches(|c: char| ",{}|".contains(c));
            if token.is_empty()
                || token.starts_with(':')
                || token.chars().all(|c| c.is_ascii_digit())
            {
                continue;
            }
            if token.ends_with(".new") || token.contains(".new(") {
                continue;
            }
            if token.contains('.') {
                let clean = token.trim_end_matches(|c: char| "(){},".contains(c));
                if !clean.is_empty() {
                    out.push(clean.into());
                }
            } else if token.ends_with(')') {
                let name = token.split('(').next().unwrap_or("");
                if !name.is_empty() && !is_non_call_keyword(name) {
                    out.push(name.into());
                }
            } else if token
                .chars()
                .next()
                .is_some_and(|c| c.is_lowercase() || c == '_')
                && !is_non_call_keyword(token)
            {
                out.push(token.into());
            }
        }
    }
    out
}

fn opens_block(line: &str) -> bool {
    line.contains(" do") || line.ends_with("do")
}

fn simple_call(line: &str) -> Option<(String, Vec<String>)> {
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    if close <= open {
        return None;
    }
    let name = line[..open].trim();
    if name.contains(' ') || name.contains('.') {
        return None;
    }
    let args = line[open + 1..close]
        .split(',')
        .map(|a| a.trim().to_string())
        .collect();
    Some((name.into(), args))
}

fn method_params(source: &str, method_q: &str) -> Vec<String> {
    let name = method_q.split(['#', '.']).next_back().unwrap_or(method_q);
    source
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let (found, params, _) = method_open(line)?;
            (found == name).then_some(params)
        })
        .unwrap_or_default()
}

fn current_source(frames: &[Frame]) -> Option<(String, String, String)> {
    let method = frames.iter().rev().find_map(|f| match f {
        Frame::Method {
            id,
            qualified,
            owner,
        } => Some((id.clone(), qualified.clone(), owner.clone())),
        _ => None,
    })?;
    if let Some(Frame::Block { id }) = frames.last() {
        Some((id.clone(), method.1, method.2))
    } else {
        Some(method)
    }
}

fn find_top_level_method(records: &[FactRecord], name: &str) -> Option<String> {
    records.iter().find_map(|r| match r {
        FactRecord::Node(n) if n.kind == "method" && n.qualified_name == name => {
            Some(n.qualified_name.clone())
        }
        _ => None,
    })
}
