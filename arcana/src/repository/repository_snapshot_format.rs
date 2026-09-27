use std::fmt::Write as FmtWrite;
use std::path::PathBuf;

use super::RepositorySnapshotError;
use super::repository_snapshot_format_support::{
    decimal, hex, validate_path, validate_store_version, validate_text,
};

pub const REPOSITORY_MANIFEST_VERSION: u64 = 2;

const FIELDS: [&str; 15] = [
    "version",
    "snapshot_id",
    "created_unix_seconds",
    "repository_id",
    "adapter_name",
    "adapter_version",
    "repository_store_version",
    "node_count",
    "edge_count",
    "unresolved_count",
    "graph_snapshot_id",
    "graph_manifest_checksum",
    "repository_store_checksum",
    "graph_manifest_file",
    "repository_store_file",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySnapshotManifest {
    pub snapshot_id: u64,
    pub created_unix_seconds: u64,
    pub repository_id: u64,
    pub adapter_name: String,
    pub adapter_version: String,
    pub repository_store_version: u16,
    pub node_count: u32,
    pub edge_count: u64,
    pub unresolved_count: u64,
    pub graph_snapshot_id: u64,
    pub graph_manifest_checksum: u64,
    pub repository_store_checksum: u64,
    pub graph_manifest_file: PathBuf,
    pub repository_store_file: PathBuf,
}

impl RepositorySnapshotManifest {
    pub fn encode(&self) -> Result<String, RepositorySnapshotError> {
        validate_text("adapter_name", &self.adapter_name)?;
        validate_text("adapter_version", &self.adapter_version)?;
        validate_store_version(self.repository_store_version)?;
        for (field, path) in [
            ("graph_manifest_file", &self.graph_manifest_file),
            ("repository_store_file", &self.repository_store_file),
        ] {
            validate_path(field, path)?;
        }

        let mut output = String::new();
        macro_rules! field {
            ($name:literal, $value:expr) => {
                writeln!(output, concat!($name, "={}"), $value)
                    .expect("writing to String cannot fail")
            };
        }
        field!("version", REPOSITORY_MANIFEST_VERSION);
        field!("snapshot_id", format_args!("{:016x}", self.snapshot_id));
        field!("created_unix_seconds", self.created_unix_seconds);
        field!("repository_id", format_args!("{:016x}", self.repository_id));
        field!("adapter_name", self.adapter_name);
        field!("adapter_version", self.adapter_version);
        field!("repository_store_version", self.repository_store_version);
        field!("node_count", self.node_count);
        field!("edge_count", self.edge_count);
        field!("unresolved_count", self.unresolved_count);
        field!(
            "graph_snapshot_id",
            format_args!("{:016x}", self.graph_snapshot_id)
        );
        field!(
            "graph_manifest_checksum",
            format_args!("{:016x}", self.graph_manifest_checksum)
        );
        field!(
            "repository_store_checksum",
            format_args!("{:016x}", self.repository_store_checksum)
        );
        field!("graph_manifest_file", self.graph_manifest_file.display());
        field!(
            "repository_store_file",
            self.repository_store_file.display()
        );
        Ok(output)
    }

    pub fn decode(text: &str) -> Result<Self, RepositorySnapshotError> {
        if !text.ends_with('\n') {
            return Err(RepositorySnapshotError::MalformedManifest(
                "missing final newline",
            ));
        }
        let lines = text[..text.len() - 1].split('\n').collect::<Vec<_>>();
        if lines.len() != FIELDS.len() {
            return Err(RepositorySnapshotError::MalformedManifest(
                "wrong field count",
            ));
        }
        let mut values = Vec::with_capacity(FIELDS.len());
        for (index, line) in lines.iter().enumerate() {
            let (field, value) =
                line.split_once('=')
                    .ok_or(RepositorySnapshotError::MalformedManifest(
                        "malformed field",
                    ))?;
            if field != FIELDS[index] {
                return Err(RepositorySnapshotError::MalformedManifest(
                    "field order mismatch",
                ));
            }
            values.push(value);
        }

        let version = decimal(values[0])?;
        if version != REPOSITORY_MANIFEST_VERSION {
            return Err(RepositorySnapshotError::UnsupportedManifestVersion(version));
        }
        let repository_store_version = u16::try_from(decimal(values[6])?).map_err(|_| {
            RepositorySnapshotError::MalformedManifest("repository store version overflow")
        })?;
        validate_store_version(repository_store_version)?;

        let manifest = Self {
            snapshot_id: hex(values[1])?,
            created_unix_seconds: decimal(values[2])?,
            repository_id: hex(values[3])?,
            adapter_name: values[4].to_owned(),
            adapter_version: values[5].to_owned(),
            repository_store_version,
            node_count: u32::try_from(decimal(values[7])?)
                .map_err(|_| RepositorySnapshotError::MalformedManifest("node count overflow"))?,
            edge_count: decimal(values[8])?,
            unresolved_count: decimal(values[9])?,
            graph_snapshot_id: hex(values[10])?,
            graph_manifest_checksum: hex(values[11])?,
            repository_store_checksum: hex(values[12])?,
            graph_manifest_file: PathBuf::from(values[13]),
            repository_store_file: PathBuf::from(values[14]),
        };
        validate_text("adapter_name", &manifest.adapter_name)?;
        validate_text("adapter_version", &manifest.adapter_version)?;
        for (field, path) in [
            ("graph_manifest_file", &manifest.graph_manifest_file),
            ("repository_store_file", &manifest.repository_store_file),
        ] {
            validate_path(field, path)?;
        }
        Ok(manifest)
    }
}
