use std::{fs, path::Path};

use ta_protocol::wire::PluginCapability;

use super::*;

fn package(root: &Path, manifest: &str, entrypoint: &str) {
    fs::create_dir_all(root).expect("package directory");
    fs::write(root.join("manifest.json"), manifest).expect("manifest");
    fs::write(root.join("plugin.js"), entrypoint).expect("entrypoint");
}

#[test]
fn inspection_is_stable_for_exact_manifest_and_entrypoint_bytes() {
    let temp = tempfile::tempdir().expect("tempdir");
    package(
        temp.path(),
        r#"{"id":"example","version":"1.2.3","entrypoint":"plugin.js","capabilities":["workspaceRead","network"]}"#,
        "export default 1;",
    );
    let first = PluginPackage::inspect(temp.path()).expect("inspection");
    let second = PluginPackage::inspect(temp.path()).expect("inspection");
    assert_eq!(first.inspection(), second.inspection());
}

#[test]
fn duplicate_requested_capability_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    package(
        temp.path(),
        r#"{"id":"example","version":"1.2.3","entrypoint":"plugin.js","capabilities":["network","network"]}"#,
        "export default 1;",
    );
    assert!(PluginPackage::inspect(temp.path()).is_err());
}

#[test]
fn package_rejects_extra_files_and_nested_directories() {
    let temp = tempfile::tempdir().expect("tempdir");
    package(
        temp.path(),
        r#"{"id":"example","version":"1.2.3","entrypoint":"plugin.js","capabilities":["network"]}"#,
        "export default 1;",
    );
    fs::write(temp.path().join("extra.txt"), "not allowed").expect("extra file");
    assert!(PluginPackage::inspect(temp.path()).is_err());
    fs::remove_file(temp.path().join("extra.txt")).expect("remove extra file");
    fs::create_dir(temp.path().join("nested")).expect("nested directory");
    assert!(PluginPackage::inspect(temp.path()).is_err());
}

#[test]
fn semantic_version_is_exact_and_supports_prerelease_plus_build() {
    let temp = tempfile::tempdir().expect("tempdir");
    package(
        temp.path(),
        r#"{"id":"example","version":"1.2.3-alpha.1+build.5","entrypoint":"plugin.js","capabilities":["network"]}"#,
        "export default 1;",
    );
    assert!(PluginPackage::inspect(temp.path()).is_ok());
    fs::write(
        temp.path().join("manifest.json"),
        r#"{"id":"example","version":"1.2.3-","entrypoint":"plugin.js","capabilities":["network"]}"#,
    )
    .expect("invalid manifest");
    assert!(PluginPackage::inspect(temp.path()).is_err());
}

#[test]
fn granted_capabilities_are_domain_validated_and_canonicalized() {
    let temp = tempfile::tempdir().expect("tempdir");
    package(
        temp.path(),
        r#"{"id":"example","version":"1.2.3","entrypoint":"plugin.js","capabilities":["workspaceRead","network"]}"#,
        "export default 1;",
    );
    let package = PluginPackage::inspect(temp.path()).expect("inspection");

    assert_eq!(
        package
            .canonical_granted_capabilities(&[
                PluginCapability::Network,
                PluginCapability::WorkspaceRead,
            ])
            .expect("canonical grants"),
        vec![PluginCapability::WorkspaceRead, PluginCapability::Network]
    );
    assert_eq!(
        package
            .canonical_granted_capabilities(&[])
            .expect("explicit empty grant"),
        Vec::<PluginCapability>::new()
    );
    assert!(matches!(
        package.canonical_granted_capabilities(&[
            PluginCapability::Network,
            PluginCapability::Network,
        ]),
        Err(PluginPackageError::InvalidCapabilityGrant)
    ));
    assert!(matches!(
        package.canonical_granted_capabilities(&[PluginCapability::ProcessExecute]),
        Err(PluginPackageError::InvalidCapabilityGrant)
    ));
}
