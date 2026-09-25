pub fn mask_comments(content: &str, language: &str) -> String {
    let bytes = content.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    let mut block_end: Option<&[u8]> = None;
    let mut quote: Option<u8> = None;
    let mut escaped = false;

    while index < bytes.len() {
        if let Some(end) = block_end {
            if bytes[index..].starts_with(end) {
                output.extend(std::iter::repeat_n(b' ', end.len()));
                index += end.len();
                block_end = None;
                continue;
            }
            output.push(if matches!(bytes[index], b'\n' | b'\r') {
                bytes[index]
            } else {
                b' '
            });
            index += 1;
            continue;
        }

        let current = bytes[index];
        if let Some(active_quote) = quote {
            output.push(current);
            if escaped {
                escaped = false;
            } else if current == b'\\' {
                escaped = true;
            } else if current == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }

        if matches!(current, b'\'' | b'"' | b'\x60') {
            quote = Some(current);
            output.push(current);
            index += 1;
            continue;
        }

        if uses_c_comments(language) && bytes[index..].starts_with(b"/*") {
            output.extend_from_slice(b"  ");
            index += 2;
            block_end = Some(b"*/");
            continue;
        }
        if language == "ps1" && bytes[index..].starts_with(b"<#") {
            output.extend_from_slice(b"  ");
            index += 2;
            block_end = Some(b"#>");
            continue;
        }
        if language == "lua" && bytes[index..].starts_with(b"--[[") {
            output.extend_from_slice(b"    ");
            index += 4;
            block_end = Some(b"]]");
            continue;
        }
        if (uses_slash_comments(language) && bytes[index..].starts_with(b"//"))
            || (uses_dash_comments(language) && bytes[index..].starts_with(b"--"))
            || (uses_hash_comments(language) && current == b'#')
        {
            while index < bytes.len() && !matches!(bytes[index], b'\n' | b'\r') {
                output.push(b' ');
                index += 1;
            }
            continue;
        }

        output.push(current);
        index += 1;
    }

    String::from_utf8(output).expect("masking preserves UTF-8")
}

pub fn mask_strings(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let mut index = 0;

    while index < bytes.len() {
        let current = bytes[index];
        if quote.is_none() {
            if matches!(current, b'\'' | b'"' | b'\x60') {
                quote = Some(current);
                output.push(b' ');
            } else {
                output.push(current);
            }
            index += 1;
            continue;
        }

        if matches!(current, b'\n' | b'\r') {
            output.push(current);
            index += 1;
            continue;
        }

        output.push(b' ');
        if escaped {
            escaped = false;
        } else if current == b'\\' {
            escaped = true;
        } else if quote == Some(current) {
            if bytes.get(index + 1) == Some(&current) {
                output.push(b' ');
                index += 1;
            } else {
                quote = None;
            }
        }
        index += 1;
    }

    String::from_utf8(output).expect("masking preserves UTF-8")
}

fn uses_c_comments(language: &str) -> bool {
    matches!(
        language,
        "c" | "cc"
            | "cpp"
            | "h"
            | "hh"
            | "hpp"
            | "cs"
            | "java"
            | "kt"
            | "kts"
            | "swift"
            | "m"
            | "mm"
            | "dart"
            | "groovy"
            | "scala"
            | "sol"
            | "proto"
            | "php"
            | "sql"
            | "v"
            | "sv"
            | "zig"
    )
}

fn uses_slash_comments(language: &str) -> bool {
    uses_c_comments(language)
}

fn uses_dash_comments(language: &str) -> bool {
    matches!(language, "lua" | "sql" | "hs" | "lhs" | "elm")
}

fn uses_hash_comments(language: &str) -> bool {
    matches!(
        language,
        "sh" | "bash" | "fish" | "ps1" | "pl" | "pm" | "r" | "rb" | "cr" | "nim" | "nims"
    )
}
