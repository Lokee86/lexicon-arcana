use regex::Regex;
use std::sync::LazyLock;

use super::gradle::offset_span;
use super::model::DependencyEvidence;

static TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<\s*(/?)\s*([A-Za-z0-9_.:-]+)(?:\s[^>]*)?>").unwrap());

pub fn parse(path: &str, content: &[u8]) -> (Vec<DependencyEvidence>, Option<String>) {
    let Ok(text) = std::str::from_utf8(content) else {
        return (Vec::new(), Some("malformed pom.xml".into()));
    };
    let mut stack = Vec::<String>::new();
    let mut result = Vec::new();
    let mut dependency_start = None::<usize>;
    let mut dependency_depth = 0_usize;
    let mut fields = std::collections::BTreeMap::<String, String>::new();
    let mut unsupported = false;
    let mut field = None::<(String, usize)>;

    for capture in TAG.captures_iter(text) {
        let Some(whole) = capture.get(0) else {
            continue;
        };
        let closing = capture.get(1).is_some_and(|value| value.as_str() == "/");
        let name = capture.get(2).map_or("", |value| value.as_str()).to_owned();

        if !closing {
            if dependency_start.is_none()
                && name == "dependency"
                && stack.as_slice() == ["project", "dependencies"]
            {
                dependency_start = Some(whole.start());
                dependency_depth = stack.len() + 1;
                fields.clear();
                unsupported = false;
            } else if dependency_start.is_some() && stack.len() == dependency_depth {
                if matches!(
                    name.as_str(),
                    "groupId" | "artifactId" | "version" | "scope" | "optional"
                ) {
                    field = Some((name.clone(), whole.end()));
                } else if matches!(name.as_str(), "classifier" | "systemPath" | "type") {
                    unsupported = true;
                }
            } else if dependency_start.is_some() && field.is_some() {
                unsupported = true;
            }
            stack.push(name);
            continue;
        }

        if let Some((field_name, value_start)) = field.take() {
            if field_name == name && stack.len() == dependency_depth + 1 {
                let value = text[value_start..whole.start()].trim();
                fields.insert(field_name, xml_unescape(value));
            } else {
                field = Some((field_name, value_start));
            }
        }

        if dependency_start.is_some() && name == "dependency" && stack.len() == dependency_depth {
            let start = dependency_start.take().unwrap();
            let end = whole.end();
            result.push(evidence(path, content, start, end, &fields, unsupported));
            dependency_depth = 0;
            field = None;
        }
        if !stack.is_empty() {
            stack.pop();
        }
    }

    if dependency_start.is_some() {
        return (result, Some("malformed pom.xml".into()));
    }
    (result, None)
}

fn evidence(
    path: &str,
    content: &[u8],
    start: usize,
    end: usize,
    fields: &std::collections::BTreeMap<String, String>,
    unsupported: bool,
) -> DependencyEvidence {
    let group = fields.get("groupId").cloned().unwrap_or_default();
    let artifact = fields.get("artifactId").cloned().unwrap_or_default();
    let version = fields.get("version").cloned().unwrap_or_default();
    let scope = fields.get("scope").cloned().unwrap_or_default();
    let optional_text = fields.get("optional").cloned().unwrap_or_default();
    let optional = optional_text == "true";
    let mut coordinate = format!("{group}:{artifact}");
    if !version.is_empty() {
        coordinate.push(':');
        coordinate.push_str(&version);
    }
    let literal = |value: &str, required: bool| {
        (!required || !value.is_empty())
            && !value
                .as_bytes()
                .windows(2)
                .any(|window| window.first() == Some(&b'$') && window.get(1) == Some(&b'{'))
            && !value.chars().any(char::is_whitespace)
            && !value.chars().any(|value| "<>/\\".contains(value))
    };
    let optional_valid =
        optional_text.is_empty() || matches!(optional_text.as_str(), "true" | "false");
    let resolved = !unsupported
        && literal(&group, true)
        && literal(&artifact, true)
        && literal(&version, false)
        && literal(&scope, false)
        && literal(&optional_text, false)
        && scope != "import"
        && optional_valid;

    DependencyEvidence {
        artifact,
        configuration: scope.clone(),
        coordinate,
        expression: String::from_utf8_lossy(&content[start..end]).trim().into(),
        group,
        optional,
        resolved,
        scope,
        span: offset_span(path, content, start, end),
        version,
    }
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
