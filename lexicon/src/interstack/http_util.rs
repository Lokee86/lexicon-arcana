use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use super::http::HttpProducer;

pub(crate) static QUOTED_HTTP_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["']([^"']*(?:https?://|/api/|/internal/|/health\b|/ws\b)[^"']*)["']"#).unwrap()
});
static PLACEHOLDER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{[^}/]+\}|:[A-Za-z_][A-Za-z0-9_]*|%\{[^}]+\}|%s|#\{[^}]+\}"#).unwrap()
});
static CONFIGURED_HTTP_METHOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)["']method["']\s*:\s*["'](GET|POST|PUT|PATCH|DELETE)["']"#).unwrap()
});
static HTTP_CLIENT_METHOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^A-Za-z0-9_])(get|post|put|patch|delete)(?:_json)?\s*\(").unwrap()
});
pub(crate) static HTTP_HELPER_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap());

pub(crate) fn normalize_http_path(value: &str) -> String {
    let mut value = value.trim().replace(r"\/", "/");
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("res://") || lower.starts_with("user://") {
        return String::new();
    }

    if let Some(scheme) = value.find("://") {
        let after = &value[scheme + 3..];
        value = after
            .find('/')
            .map(|index| after[index..].to_owned())
            .unwrap_or_else(|| "/".into());
    } else {
        for marker in ["/api/", "/internal/", "/health", "/ws"] {
            if let Some(index) = value.find(marker) {
                value = value[index..].to_owned();
                break;
            }
        }
    }

    if let Some(index) = value.find(['?', '#']) {
        value.truncate(index);
    }
    value = PLACEHOLDER_PATTERN
        .replace_all(&value, "{param}")
        .into_owned();
    if !value.starts_with('/') {
        value.insert(0, '/');
    }
    let value = clean_path(&value.replace("//", "/"));
    if value == "/" { String::new() } else { value }
}

pub(crate) fn http_path_shape(value: &str) -> String {
    PLACEHOLDER_PATTERN.replace_all(value, "{}").into_owned()
}

pub(crate) fn display_http_expression(method: &str, route: &str) -> String {
    format!("{} {route}", if method.is_empty() { "*" } else { method })
}

pub(crate) fn looks_like_http_client_call(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("handlefunc(") || lower.contains(".handler(") {
        return false;
    }
    HTTP_CLIENT_METHOD.is_match(line)
        || lower.contains("request(")
        || lower.contains("fetch(")
        || lower.contains("axios.")
        || lower.contains("requests.")
        || lower.contains("httpclient")
        || lower.contains("net::http")
}

pub(crate) fn is_http_provider_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["_path", "_url", "_endpoint"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

pub(crate) fn unique_http_provider(providers: &[HttpProducer]) -> Option<HttpProducer> {
    let mut unique = HashMap::new();
    for provider in providers {
        unique.insert(provider.shape.clone(), provider.clone());
    }
    (unique.len() == 1).then(|| unique.into_values().next().expect("one provider"))
}

pub(crate) fn http_call_block(lines: &[String], start: usize) -> (String, usize) {
    let mut end = start;
    let mut balance = 0_i32;
    let mut started = false;
    let mut parts = Vec::new();
    while end < lines.len() && end < start + 12 {
        let line = &lines[end];
        parts.push(line.trim().to_owned());
        balance += line.matches('(').count() as i32;
        balance -= line.matches(')').count() as i32;
        started |= line.contains('(');
        if started && balance <= 0 {
            break;
        }
        end += 1;
    }
    (parts.join("\n"), end)
}

pub(crate) fn compact_evidence(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn http_method_from_line(line: &str) -> String {
    if let Some(capture) = HTTP_CLIENT_METHOD.captures(line) {
        return capture[1].to_ascii_uppercase();
    }
    let upper = line.to_ascii_uppercase();
    let lower = line.to_ascii_lowercase();
    for method in ["DELETE", "PATCH", "POST", "PUT", "GET"] {
        if upper.contains(&format!("METHOD_{method}"))
            || upper.contains(&format!("METHOD{method}"))
            || lower.contains(&format!(".{}(", method.to_ascii_lowercase()))
        {
            return method.to_owned();
        }
    }
    CONFIGURED_HTTP_METHOD
        .captures(line)
        .map(|capture| capture[1].to_ascii_uppercase())
        .unwrap_or_default()
}

fn clean_path(value: &str) -> String {
    let mut parts = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}
