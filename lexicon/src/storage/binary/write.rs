use super::common::{code, is_sha256};
use crate::SourceSpan;
use std::collections::BTreeMap;

pub(crate) fn uvarint(output: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        output.push((value as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

pub(crate) fn bytes(output: &mut Vec<u8>, value: &[u8]) {
    uvarint(output, value.len() as u64);
    output.extend_from_slice(value);
}

pub(crate) fn string_ref(output: &mut Vec<u8>, table: &BTreeMap<String, u64>, value: &str) {
    uvarint(output, table[value]);
}

pub(crate) fn identity(output: &mut Vec<u8>, table: &BTreeMap<String, u64>, value: &str) {
    if is_sha256(value) {
        output.push(1);
        for index in (7..71).step_by(2) {
            output.push(hex_byte(&value[index..index + 2]));
        }
    } else {
        output.push(0);
        string_ref(output, table, value);
    }
}

pub(crate) fn code_or_string(
    output: &mut Vec<u8>,
    table: &BTreeMap<String, u64>,
    value: &str,
    values: &[&str],
) {
    let encoded = code(value, values);
    uvarint(output, encoded);
    if encoded == 0 {
        string_ref(output, table, value);
    }
}

pub(crate) fn factored(
    output: &mut Vec<u8>,
    table: &BTreeMap<String, u64>,
    value: &str,
    object_owner: &str,
) {
    match value {
        "" => uvarint(output, 0),
        value if value == object_owner => uvarint(output, 1),
        _ => {
            uvarint(output, 2);
            string_ref(output, table, value);
        }
    }
}

pub(crate) fn qname(
    output: &mut Vec<u8>,
    table: &BTreeMap<String, u64>,
    value: &str,
    name: &str,
    path: &str,
    object_owner: &str,
) {
    if value == name {
        uvarint(output, 1);
    } else if value == path {
        uvarint(output, 2);
    } else if value == object_owner {
        uvarint(output, 3);
    } else {
        uvarint(output, 0);
        string_ref(output, table, value);
    }
}

pub(crate) fn span(output: &mut Vec<u8>, table: &BTreeMap<String, u64>, span: Option<&SourceSpan>) {
    let Some(span) = span else {
        output.push(0);
        return;
    };
    output.push(1);
    string_ref(output, table, &span.path);
    uvarint(output, span.start_line);
    uvarint(output, span.start_column);
    uvarint(output, span.end_line);
    uvarint(output, span.end_column);
}

pub(crate) fn section(output: &mut Vec<u8>, value: &[u8]) {
    bytes(output, value);
}

fn hex_byte(value: &str) -> u8 {
    u8::from_str_radix(value, 16).expect("validated SHA-256 hexadecimal")
}
