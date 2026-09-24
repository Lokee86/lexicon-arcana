use sha2::{Digest, Sha256};

pub(crate) fn synthetic_path(category: &str, identity: &str) -> String {
    let digest = Sha256::digest(identity.as_bytes());
    let prefix = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("@interstack/{category}/{prefix}")
}

pub(crate) fn camelize(value: &str) -> String {
    value
        .split(['_', '-', '/'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            first.to_uppercase().collect::<String>() + chars.as_str()
        })
        .collect()
}

pub(crate) fn camel_to_kebab(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            output.push('-');
        }
        output.extend(character.to_lowercase());
    }
    output
}

pub(crate) fn last_identifier(value: &str) -> String {
    let value = value.split('(').next().unwrap_or(value).trim();
    value
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .rfind(|part| !part.is_empty())
        .unwrap_or_default()
        .to_owned()
}
