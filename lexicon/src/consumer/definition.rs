use serde_json::Value;

use crate::LexiconError;

pub const CONSUMER_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerDefinition {
    pub version: u64,
    pub command: String,
    pub args: Vec<String>,
    pub timeout_nanos: u64,
}

impl ConsumerDefinition {
    pub fn parse(data: &[u8]) -> Result<Self, LexiconError> {
        let value: Value = serde_json::from_slice(data)
            .map_err(|error| LexiconError::new(format!("decode Lexicon consumer: {error}")))?;
        let object = value
            .as_object()
            .ok_or_else(|| LexiconError::new("Lexicon consumer definition is not an object"))?;
        let version = object.get("version").and_then(Value::as_u64).unwrap_or(0);
        let command = object
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let args = object
            .get("args")
            .map(parse_args)
            .transpose()?
            .unwrap_or_default();
        let timeout_nanos = object
            .get("timeout")
            .map(parse_timeout)
            .transpose()?
            .unwrap_or(0);
        let definition = Self {
            version,
            command,
            args,
            timeout_nanos,
        };
        definition.validate()?;
        Ok(definition)
    }

    pub fn validate(&self) -> Result<(), LexiconError> {
        if self.version != CONSUMER_VERSION {
            return Err(LexiconError::new(format!(
                "unsupported Lexicon consumer version {}",
                self.version
            )));
        }
        if self.command.trim().is_empty() {
            return Err(LexiconError::new("Lexicon consumer has no command"));
        }
        Ok(())
    }
}

fn parse_args(value: &Value) -> Result<Vec<String>, LexiconError> {
    let values = value
        .as_array()
        .ok_or_else(|| LexiconError::new("Lexicon consumer args must be an array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| LexiconError::new("Lexicon consumer args must be strings"))
        })
        .collect()
}

fn parse_timeout(value: &Value) -> Result<u64, LexiconError> {
    match value {
        Value::Null => Ok(0),
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| LexiconError::new("consumer timeout must not be negative")),
        Value::String(text) => parse_duration(text),
        _ => Err(LexiconError::new(
            "consumer timeout must be a duration string",
        )),
    }
}

fn parse_duration(text: &str) -> Result<u64, LexiconError> {
    if text == "0" {
        return Ok(0);
    }
    if text.is_empty() || text.starts_with('-') {
        return Err(LexiconError::new(format!(
            "invalid consumer timeout {text:?}"
        )));
    }

    let bytes = text.as_bytes();
    let mut offset = 0;
    let mut total = 0_f64;
    while offset < bytes.len() {
        let start = offset;
        while offset < bytes.len() && (bytes[offset].is_ascii_digit() || bytes[offset] == b'.') {
            offset += 1;
        }
        if start == offset {
            return Err(LexiconError::new(format!(
                "invalid consumer timeout {text:?}"
            )));
        }
        let number: f64 = text[start..offset]
            .parse()
            .map_err(|_| LexiconError::new(format!("invalid consumer timeout {text:?}")))?;
        let unit_start = offset;
        while offset < bytes.len() && !bytes[offset].is_ascii_digit() && bytes[offset] != b'.' {
            offset += 1;
        }
        let unit = &text[unit_start..offset];
        let multiplier = match unit {
            "ns" => 1_f64,
            "us" | "µs" | "μs" => 1_000_f64,
            "ms" => 1_000_000_f64,
            "s" => 1_000_000_000_f64,
            "m" => 60_000_000_000_f64,
            "h" => 3_600_000_000_000_f64,
            _ => {
                return Err(LexiconError::new(format!(
                    "invalid consumer timeout {text:?}"
                )));
            }
        };
        total += number * multiplier;
    }
    if !total.is_finite() || total < 0.0 || total > u64::MAX as f64 {
        return Err(LexiconError::new(format!(
            "invalid consumer timeout {text:?}"
        )));
    }
    Ok(total as u64)
}
