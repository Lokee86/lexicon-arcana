use std::path::{Component, Path};

use crate::repository_store::format::FORMAT_VERSION as REPOSITORY_STORE_FORMAT_VERSION;

use super::RepositorySnapshotError;

pub(super) fn validate_store_version(version: u16) -> Result<(), RepositorySnapshotError> {
    if version == REPOSITORY_STORE_FORMAT_VERSION {
        Ok(())
    } else {
        Err(RepositorySnapshotError::UnsupportedRepositoryStoreVersion(
            version,
        ))
    }
}

pub(super) fn decimal(value: &str) -> Result<u64, RepositorySnapshotError> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return Err(RepositorySnapshotError::MalformedManifest(
            "invalid decimal",
        ));
    }
    value
        .parse()
        .map_err(|_| RepositorySnapshotError::MalformedManifest("invalid decimal"))
}

pub(super) fn hex(value: &str) -> Result<u64, RepositorySnapshotError> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(RepositorySnapshotError::MalformedManifest(
            "invalid checksum",
        ));
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| RepositorySnapshotError::MalformedManifest("invalid checksum"))
}

pub(super) fn validate_text(
    field: &'static str,
    value: &str,
) -> Result<(), RepositorySnapshotError> {
    if value.is_empty() || value.chars().any(char::is_control) || value.contains('=') {
        return Err(RepositorySnapshotError::InvalidTextField(field));
    }
    Ok(())
}

pub(super) fn validate_path(
    field: &'static str,
    path: &Path,
) -> Result<(), RepositorySnapshotError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        || path.to_str().is_none()
    {
        return Err(RepositorySnapshotError::InvalidComponentPath(field));
    }
    Ok(())
}
