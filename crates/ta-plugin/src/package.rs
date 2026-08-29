use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use ta_protocol::wire::{PluginCapability, PluginId, PluginInspection};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginPackageError {
    #[error("plugin package directory is invalid")]
    InvalidDirectory,
    #[error("plugin manifest is invalid")]
    InvalidManifest,
    #[error("plugin entrypoint is invalid")]
    InvalidEntrypoint,
    #[error("plugin package file cannot be read")]
    Read,
    #[error("plugin capability grants are invalid")]
    InvalidCapabilityGrant,
}

#[derive(Debug, Clone)]
pub struct PluginPackage {
    inspection: PluginInspection,
    manifest_bytes: Vec<u8>,
    entrypoint_name: String,
    entrypoint_bytes: Vec<u8>,
}

impl PluginPackage {
    pub fn inspect(source_path: &Path) -> Result<Self, PluginPackageError> {
        let metadata =
            fs::symlink_metadata(source_path).map_err(|_| PluginPackageError::InvalidDirectory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(PluginPackageError::InvalidDirectory);
        }
        let manifest_path = source_path.join("manifest.json");
        let manifest_bytes = read_regular(&manifest_path)?;
        let manifest: PluginManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| PluginPackageError::InvalidManifest)?;
        let plugin_id =
            PluginId::new(manifest.id).map_err(|_| PluginPackageError::InvalidManifest)?;
        validate_semver(&manifest.version)?;
        let entrypoint_path = safe_entrypoint(source_path, &manifest.entrypoint)?;
        validate_exact_package_contents(source_path, &manifest.entrypoint)?;
        let entrypoint_bytes = read_regular(&entrypoint_path)?;
        let requested_capabilities = canonical_capabilities(manifest.capabilities)?;
        let digest_sha256 = digest(&manifest_bytes, &entrypoint_bytes);
        Ok(Self {
            inspection: PluginInspection {
                plugin_id,
                version: manifest.version,
                digest_sha256,
                requested_capabilities,
            },
            manifest_bytes,
            entrypoint_name: manifest.entrypoint,
            entrypoint_bytes,
        })
    }

    pub fn inspection(&self) -> &PluginInspection {
        &self.inspection
    }
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }
    pub fn entrypoint_name(&self) -> &str {
        &self.entrypoint_name
    }
    pub fn entrypoint_bytes(&self) -> &[u8] {
        &self.entrypoint_bytes
    }

    /// Validates an explicit user grant against the capabilities requested by
    /// this exact inspected package. The returned collection is canonical so
    /// every durable installation has one stable grant representation.
    pub fn canonical_granted_capabilities(
        &self,
        granted_capabilities: &[PluginCapability],
    ) -> Result<Vec<PluginCapability>, PluginPackageError> {
        let granted = granted_capabilities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if granted.len() != granted_capabilities.len()
            || !granted
                .iter()
                .all(|capability| self.inspection.requested_capabilities.contains(capability))
        {
            return Err(PluginPackageError::InvalidCapabilityGrant);
        }
        Ok(granted.into_iter().collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginManifest {
    id: String,
    version: String,
    entrypoint: String,
    capabilities: Vec<PluginCapability>,
}

fn read_regular(path: &Path) -> Result<Vec<u8>, PluginPackageError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PluginPackageError::Read)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(PluginPackageError::InvalidEntrypoint);
    }
    fs::read(path).map_err(|_| PluginPackageError::Read)
}

fn safe_entrypoint(root: &Path, entrypoint: &str) -> Result<PathBuf, PluginPackageError> {
    let path = Path::new(entrypoint);
    if entrypoint.is_empty()
        || path.is_absolute()
        || path.components().count() != 1
        || entrypoint == "manifest.json"
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(PluginPackageError::InvalidEntrypoint);
    }
    Ok(root.join(path))
}

fn validate_exact_package_contents(
    root: &Path,
    entrypoint: &str,
) -> Result<(), PluginPackageError> {
    let expected = HashSet::from(["manifest.json".to_string(), entrypoint.to_string()]);
    if expected.len() != 2 {
        return Err(PluginPackageError::InvalidEntrypoint);
    }
    let actual = fs::read_dir(root)
        .map_err(|_| PluginPackageError::InvalidDirectory)?
        .map(|entry| {
            let entry = entry.map_err(|_| PluginPackageError::InvalidDirectory)?;
            let file_name = entry
                .file_name()
                .into_string()
                .map_err(|_| PluginPackageError::InvalidDirectory)?;
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| PluginPackageError::InvalidDirectory)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(PluginPackageError::InvalidDirectory);
            }
            Ok(file_name)
        })
        .collect::<Result<HashSet<_>, _>>()?;
    if actual != expected {
        return Err(PluginPackageError::InvalidDirectory);
    }
    Ok(())
}

fn canonical_capabilities(
    capabilities: Vec<PluginCapability>,
) -> Result<Vec<PluginCapability>, PluginPackageError> {
    let requested_count = capabilities.len();
    let unique = capabilities.into_iter().collect::<BTreeSet<_>>();
    if unique.is_empty() || unique.len() != requested_count {
        return Err(PluginPackageError::InvalidManifest);
    }
    Ok(unique.into_iter().collect())
}

fn validate_semver(value: &str) -> Result<(), PluginPackageError> {
    semver::Version::parse(value)
        .map(|_| ())
        .map_err(|_| PluginPackageError::InvalidManifest)
}

fn digest(manifest: &[u8], entrypoint: &[u8]) -> String {
    let mut hasher = Sha256::new();
    for bytes in [manifest, entrypoint] {
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}
