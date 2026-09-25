use std::sync::LazyLock;

use regex::Regex;

use super::facts::Facts;
use super::mask::{mask_comments, mask_strings};

static TYPE_DECLARATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"(?i)^\s*((public|private|protected|internal|static|final|abstract|sealed|open|export|extern|unsafe|pub|data|value|annotation)\s+)*(class|struct|interface|trait|enum|record|protocol|union|namespace|module|type|object)\s+([A-Za-z_][A-Za-z0-9_]*)"
).unwrap()
});
static KEYWORD_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"(?i)^\s*((public|private|protected|internal|static|final|abstract|open|export|extern|async|unsafe|pub)\s+)*(func|function|fn|def|sub|proc|procedure|fun)\s+([A-Za-z_][A-Za-z0-9_]*)\s*[<(]"
).unwrap()
});
static C_STYLE_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"^\s*((public|private|protected|internal|static|final|abstract|virtual|override|extern|async|unsafe|inline|constexpr|synchronized|native)\s+)*([A-Za-z_][A-Za-z0-9_:<>,\[\]*&?.]*\s+)+([~A-Za-z_][A-Za-z0-9_:]*)\s*\([^;{}]*\)\s*(const\b\s*)?(noexcept\b\s*)?(->\s*[^;{]+\s*)?(\{.*|=>.*)?$"
).unwrap()
});

static PREPROCESSOR_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*#\s*(include|import)\s*[<"]([^>"]+)[>"]"#).unwrap());
static FROM_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*from\s+([A-Za-z0-9_./:\-]+)\s+import\b").unwrap());
static ASSIGNED_REQUIRE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^\s*(local\s+)?[A-Za-z_][A-Za-z0-9_]*\s*=\s*require\s*\(\s*["']([^"']+)["']"#)
        .unwrap()
});
static REQUIRE_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^\s*(require|require_once|include|include_once)\s*\(?\s*["']([^"']+)["']"#)
        .unwrap()
});
static USING_NAMESPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*using\s+namespace\s+([A-Za-z0-9_:.\-]+)").unwrap());
static KEYWORD_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^\s*(import|use|using|require|include)\s+["']?([A-Za-z0-9_./:\\\-]+)"#)
        .unwrap()
});
static PROTO_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^\s*import\s+(public\s+|weak\s+)?["']([^"']+)["']"#).unwrap()
});
static POWERSHELL_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)^\s*Import-Module\s+["']?([A-Za-z0-9_./:\\\-]+)"#).unwrap());

static LUA_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(local\s+)?function\s+([A-Za-z_][A-Za-z0-9_.:]*)\s*\(").unwrap()
});
static SHELL_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(function\s+)?([A-Za-z_][A-Za-z0-9_-]*)\s*(\(\s*\))?\s*\{").unwrap()
});
static POWERSHELL_FUNCTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*function\s+([A-Za-z_][A-Za-z0-9_-]*)\b").unwrap());
static OBJECTIVE_C_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*@(interface|protocol|implementation)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
});
static OBJECTIVE_C_METHOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[-+]\s*\([^)]*\)\s*([A-Za-z_][A-Za-z0-9_]*)").unwrap());
static PROTO_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(message|enum|service)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
});
static PROTO_RPC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*rpc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap());
static SOLIDITY_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"(?i)^\s*(abstract\s+)?(contract|interface|library|struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)"
).unwrap()
});
static SQL_DECLARATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"(?i)^\s*create\s+(or\s+replace\s+)?(table|view|type|procedure|function|trigger)\s+([A-Za-z_][A-Za-z0-9_$.]*)"
).unwrap()
});
static PERL_FUNCTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*sub\s+([A-Za-z_][A-Za-z0-9_]*)\b").unwrap());
static R_FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z.][A-Za-z0-9._]*)\s*(<-|=)\s*function\s*\(").unwrap()
});

pub fn parse_source(facts: &mut Facts, path: &str, module_id: &str, content: &str) {
    let language = facts_language(facts).to_owned();
    let imports = mask_comments(content, &language);
    let declarations = mask_strings(&imports);
    let import_lines = imports.lines().collect::<Vec<_>>();
    let declaration_lines = declarations.lines().collect::<Vec<_>>();

    for (index, line) in content.lines().enumerate() {
        let line_number = index as u64 + 1;
        let import_line = import_lines.get(index).copied().unwrap_or_default();
        if let Some((keyword, target)) = parse_import(&language, import_line) {
            facts.add_import(path, module_id, &keyword, &target, line_number, line);
        }
        let declaration_line = declaration_lines.get(index).copied().unwrap_or_default();
        if let Some((kind, name)) = parse_declaration(&language, declaration_line) {
            facts.add_declaration(path, module_id, kind, &name, line_number, line);
        }
    }
}

fn facts_language(facts: &Facts) -> &str {
    facts.language_suffix()
}

fn parse_import(language: &str, line: &str) -> Option<(String, String)> {
    if let Some(m) = PREPROCESSOR_IMPORT.captures(line) {
        return Some((m[1].to_ascii_lowercase(), m[2].to_owned()));
    }
    if let Some(m) = FROM_IMPORT.captures(line) {
        return Some(("from".into(), m[1].to_owned()));
    }
    if let Some(m) = ASSIGNED_REQUIRE.captures(line) {
        return Some(("require".into(), m[2].to_owned()));
    }
    if let Some(m) = REQUIRE_IMPORT.captures(line) {
        return Some((m[1].to_ascii_lowercase(), m[2].to_owned()));
    }
    if let Some(m) = USING_NAMESPACE.captures(line) {
        return Some(("using".into(), m[1].to_owned()));
    }
    if language == "proto"
        && let Some(m) = PROTO_IMPORT.captures(line)
    {
        return Some(("import".into(), m[2].to_owned()));
    }
    if language == "ps1"
        && let Some(m) = POWERSHELL_IMPORT.captures(line)
    {
        return Some(("import-module".into(), m[1].to_owned()));
    }
    KEYWORD_IMPORT.captures(line).map(|m| {
        (
            m[1].to_ascii_lowercase(),
            m[2].trim_end_matches([';', ',']).to_owned(),
        )
    })
}

fn parse_declaration(language: &str, line: &str) -> Option<(&'static str, String)> {
    match language {
        "lua" => {
            if let Some(m) = LUA_FUNCTION.captures(line) {
                return Some(("function", terminal_name(&m[2], ".:")));
            }
        }
        "sh" | "bash" | "fish" => {
            if let Some(m) = SHELL_FUNCTION.captures(line) {
                return Some(("function", m[2].to_owned()));
            }
        }
        "ps1" => {
            if let Some(m) = POWERSHELL_FUNCTION.captures(line) {
                return Some(("function", m[1].to_owned()));
            }
        }
        "m" | "mm" => {
            if let Some(m) = OBJECTIVE_C_TYPE.captures(line) {
                let kind = if m[1].eq_ignore_ascii_case("protocol") {
                    "interface"
                } else {
                    "type"
                };
                return Some((kind, m[2].to_owned()));
            }
            if let Some(m) = OBJECTIVE_C_METHOD.captures(line) {
                return Some(("function", m[1].to_owned()));
            }
        }
        "proto" => {
            if let Some(m) = PROTO_TYPE.captures(line) {
                let kind = if m[1].eq_ignore_ascii_case("service") {
                    "interface"
                } else {
                    "type"
                };
                return Some((kind, m[2].to_owned()));
            }
            if let Some(m) = PROTO_RPC.captures(line) {
                return Some(("function", m[1].to_owned()));
            }
        }
        "sol" => {
            if let Some(m) = SOLIDITY_TYPE.captures(line) {
                let kind = if m[2].eq_ignore_ascii_case("interface") {
                    "interface"
                } else {
                    "type"
                };
                return Some((kind, m[3].to_owned()));
            }
        }
        "sql" => {
            if let Some(m) = SQL_DECLARATION.captures(line) {
                let keyword = m[2].to_ascii_lowercase();
                let kind = if matches!(keyword.as_str(), "procedure" | "function" | "trigger") {
                    "function"
                } else {
                    "type"
                };
                return Some((kind, m[3].to_owned()));
            }
        }
        "pl" | "pm" => {
            if let Some(m) = PERL_FUNCTION.captures(line) {
                return Some(("function", m[1].to_owned()));
            }
        }
        "r" => {
            if let Some(m) = R_FUNCTION.captures(line) {
                return Some(("function", m[1].to_owned()));
            }
        }
        _ => {}
    }

    if let Some(m) = TYPE_DECLARATION.captures(line) {
        return Some((declaration_kind(&m[3]), m[4].to_owned()));
    }
    if let Some(m) = KEYWORD_FUNCTION.captures(line) {
        return Some(("function", m[4].to_owned()));
    }

    let trimmed = line.trim();
    let first = trimmed
        .split(|value: char| value.is_whitespace() || value == '(')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        first.as_str(),
        "if" | "for" | "while" | "switch" | "catch" | "return" | "sizeof"
    ) {
        return None;
    }
    C_STYLE_FUNCTION
        .captures(line)
        .map(|m| ("function", terminal_name(&m[4], ":")))
}

fn terminal_name(name: &str, separators: &str) -> String {
    name.rsplit(|value| separators.contains(value))
        .next()
        .unwrap_or(name)
        .to_owned()
}

fn declaration_kind(keyword: &str) -> &'static str {
    match keyword.to_ascii_lowercase().as_str() {
        "interface" | "protocol" => "interface",
        "trait" => "trait",
        "namespace" => "namespace",
        "module" => "module",
        _ => "type",
    }
}
