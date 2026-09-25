use base64::Engine;
use regex::Regex;
use std::sync::LazyLock;

static LOTUS_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<lotusscript(?:\s[^>]*)?>(.*?)</lotusscript>").unwrap());
static RAW_ITEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<rawitemdata\s+[^>]*type\s*=\s*['"]10['"][^>]*>(.*?)</rawitemdata>"#)
        .unwrap()
});

pub fn lotus_script_content(extension: &str, raw: &[u8]) -> Option<Vec<u8>> {
    if matches!(extension, ".ls" | ".lss") {
        return Some(raw.to_vec());
    }
    let trimmed = trim_ascii(raw);
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.first() != Some(&b'<') {
        return (std::str::from_utf8(raw).is_ok() && !raw.contains(&0)).then(|| raw.to_vec());
    }
    extract_dxl_source(raw)
}

fn extract_dxl_source(raw: &[u8]) -> Option<Vec<u8>> {
    let xml = std::str::from_utf8(raw).ok()?;
    let sections = LOTUS_SCRIPT
        .captures_iter(xml)
        .filter_map(|capture| capture.get(1))
        .map(|value| unescape_xml(value.as_str()).trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if !sections.is_empty() {
        return Some(sections.join("\n\n").into_bytes());
    }

    let mut best = Vec::new();
    for capture in RAW_ITEM.captures_iter(xml) {
        let Some(value) = capture.get(1) else {
            continue;
        };
        let encoded = value.as_str().split_whitespace().collect::<String>();
        let Ok(payload) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
            continue;
        };
        let Some(candidate) = lotus_script_payload(&payload) else {
            continue;
        };
        if candidate.len() > best.len() {
            best = candidate;
        }
    }
    (!best.is_empty()).then_some(best)
}

fn lotus_script_payload(payload: &[u8]) -> Option<Vec<u8>> {
    let markers: &[&[u8]] = &[
        b"'++LotusScript Development Environment",
        b"Option Public",
        b"Option Declare",
    ];
    let start = markers
        .iter()
        .filter_map(|marker| find_bytes(payload, marker))
        .min()?;
    let tail = &payload[start..];
    let end = tail
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(tail.len());
    let mut result = tail[..end].to_vec();
    for value in &mut result {
        if !matches!(*value, b'\t' | b'\n' | b'\r' | 0x20..=0x7e) {
            *value = b' ';
        }
    }
    Some(trim_ascii(&result).to_vec())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn trim_ascii(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    &value[start..end]
}

fn unescape_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
