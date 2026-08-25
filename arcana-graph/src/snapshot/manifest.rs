use std::fmt::Write as FmtWrite;
use std::path::{Component, Path, PathBuf};

use super::ManifestError;

/// The only manifest format currently understood by Arcana.
pub const MANIFEST_VERSION: u64 = 1;

const FIELD_ORDER: [&str; 11] = [
    "version",
    "snapshot_id",
    "created_unix_seconds",
    "node_count",
    "base_edge_count",
    "visible_edge_count",
    "base_dataset_checksum",
    "overlay_checksum",
    "visible_dataset_checksum",
    "base_file",
    "overlay_file",
];

/// Metadata identifying a graph snapshot and the files that compose it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotManifest {
    pub snapshot_id: u64,
    pub created_unix_seconds: u64,
    pub node_count: u32,
    pub base_edge_count: u64,
    pub visible_edge_count: u64,
    pub base_dataset_checksum: u64,
    pub overlay_checksum: u64,
    pub visible_dataset_checksum: u64,
    pub base_file: PathBuf,
    pub overlay_file: Option<PathBuf>,
}

/// Short alias for callers that do not need the snapshot-specific name.
pub type Manifest = SnapshotManifest;

impl SnapshotManifest {
    /// Encodes this manifest in its canonical, newline-terminated form.
    pub fn encode(&self) -> Result<String, ManifestError> {
        let base_file = checked_path_text("base_file", &self.base_file)?;
        let overlay_file = self
            .overlay_file
            .as_ref()
            .map(|path| checked_path_text("overlay_file", path))
            .transpose()?;
        let mut encoded = String::new();
        writeln!(encoded, "version={MANIFEST_VERSION}").expect("writing to String cannot fail");
        writeln!(encoded, "snapshot_id={:016x}", self.snapshot_id)
            .expect("writing to String cannot fail");
        writeln!(
            encoded,
            "created_unix_seconds={}",
            self.created_unix_seconds
        )
        .expect("writing to String cannot fail");
        writeln!(encoded, "node_count={}", self.node_count).expect("writing to String cannot fail");
        writeln!(encoded, "base_edge_count={}", self.base_edge_count)
            .expect("writing to String cannot fail");
        writeln!(encoded, "visible_edge_count={}", self.visible_edge_count)
            .expect("writing to String cannot fail");
        writeln!(
            encoded,
            "base_dataset_checksum={:016x}",
            self.base_dataset_checksum
        )
        .expect("writing to String cannot fail");
        writeln!(encoded, "overlay_checksum={:016x}", self.overlay_checksum)
            .expect("writing to String cannot fail");
        writeln!(
            encoded,
            "visible_dataset_checksum={:016x}",
            self.visible_dataset_checksum
        )
        .expect("writing to String cannot fail");
        writeln!(encoded, "base_file={base_file}").expect("writing to String cannot fail");
        if let Some(overlay_file) = overlay_file {
            writeln!(encoded, "overlay_file={overlay_file}")
                .expect("writing to String cannot fail");
        }
        Ok(encoded)
    }

    /// Parses one canonical manifest, rejecting unknown, repeated, or reordered fields.
    pub fn decode(text: &str) -> Result<Self, ManifestError> {
        if !text.ends_with('\n') {
            return Err(ManifestError::MissingFinalNewline);
        }

        let mut values: [Option<String>; 11] = Default::default();
        for (expected_index, line) in text[..text.len() - 1].split('\n').enumerate() {
            let (field, value) =
                line.split_once('=')
                    .ok_or_else(|| ManifestError::MalformedField {
                        field: "line",
                        value: line.to_owned(),
                    })?;
            let Some(field_index) = FIELD_ORDER.iter().position(|candidate| *candidate == field)
            else {
                return Err(ManifestError::UnknownField(field.to_owned()));
            };
            if values[field_index].is_some() {
                return Err(ManifestError::DuplicateField(field.to_owned()));
            }
            let expected = FIELD_ORDER
                .get(expected_index)
                .copied()
                .unwrap_or("end of manifest");
            if field_index != expected_index {
                if field_index > expected_index && expected_index < 10 {
                    return Err(ManifestError::MissingField(FIELD_ORDER[expected_index]));
                }
                return Err(ManifestError::InvalidFieldOrder {
                    expected,
                    found: field.to_owned(),
                });
            }
            values[field_index] = Some(value.to_owned());
        }

        let version = parse_decimal("version", required(&values, 0, "version")?)?;
        if version != MANIFEST_VERSION {
            return Err(ManifestError::UnsupportedVersion { found: version });
        }
        let snapshot_id = parse_hex("snapshot_id", required(&values, 1, "snapshot_id")?)?;
        let created_unix_seconds = parse_decimal(
            "created_unix_seconds",
            required(&values, 2, "created_unix_seconds")?,
        )?;
        let node_count = u32::try_from(parse_decimal(
            "node_count",
            required(&values, 3, "node_count")?,
        )?)
        .map_err(|_| ManifestError::MalformedField {
            field: "node_count",
            value: values[3].clone().unwrap_or_default(),
        })?;
        let base_edge_count =
            parse_decimal("base_edge_count", required(&values, 4, "base_edge_count")?)?;
        let visible_edge_count = parse_decimal(
            "visible_edge_count",
            required(&values, 5, "visible_edge_count")?,
        )?;
        let base_dataset_checksum = parse_hex(
            "base_dataset_checksum",
            required(&values, 6, "base_dataset_checksum")?,
        )?;
        let overlay_checksum = parse_hex(
            "overlay_checksum",
            required(&values, 7, "overlay_checksum")?,
        )?;
        let visible_dataset_checksum = parse_hex(
            "visible_dataset_checksum",
            required(&values, 8, "visible_dataset_checksum")?,
        )?;
        let base_file = parse_path("base_file", required(&values, 9, "base_file")?)?;
        let overlay_file = values[10]
            .as_deref()
            .map(|value| parse_path("overlay_file", value))
            .transpose()?;

        Ok(Self {
            snapshot_id,
            created_unix_seconds,
            node_count,
            base_edge_count,
            visible_edge_count,
            base_dataset_checksum,
            overlay_checksum,
            visible_dataset_checksum,
            base_file,
            overlay_file,
        })
    }
}

fn required<'a>(
    values: &'a [Option<String>; 11],
    index: usize,
    field: &'static str,
) -> Result<&'a str, ManifestError> {
    values[index]
        .as_deref()
        .ok_or(ManifestError::MissingField(field))
}

fn parse_decimal(field: &'static str, value: &str) -> Result<u64, ManifestError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ManifestError::MalformedField {
            field,
            value: value.to_owned(),
        });
    }
    value.parse().map_err(|_| ManifestError::MalformedField {
        field,
        value: value.to_owned(),
    })
}

fn parse_hex(field: &'static str, value: &str) -> Result<u64, ManifestError> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ManifestError::MalformedField {
            field,
            value: value.to_owned(),
        });
    }
    u64::from_str_radix(value, 16).map_err(|_| ManifestError::MalformedField {
        field,
        value: value.to_owned(),
    })
}

fn parse_path(field: &'static str, value: &str) -> Result<PathBuf, ManifestError> {
    let path = PathBuf::from(value);
    checked_path_text(field, &path)?;
    Ok(path)
}

fn checked_path_text<'a>(field: &'static str, path: &'a Path) -> Result<&'a str, ManifestError> {
    let text = path.to_str().ok_or(ManifestError::NonUtf8Path { field })?;
    if text.is_empty()
        || text.contains('\n')
        || text.contains('\r')
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ManifestError::InvalidPath {
            field,
            path: text.to_owned(),
        });
    }
    Ok(text)
}
