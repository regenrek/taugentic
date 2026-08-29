use std::{fs, path::Path};

use ta_plugin::PluginPackage;
use ta_protocol::wire::{
    InspectPluginPackageRequest, InstallPluginPackageRequest, InstallPluginPackageResult,
    ListPluginInstallationsResult, PluginInstallation, PluginLifecycleState,
    UninstallPluginRequest,
};
use ta_store::PersistenceStore;
use uuid::Uuid;

use super::{AppService, AppServiceError};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub(crate) fn inspect_plugin_package(
        &self,
        request: &InspectPluginPackageRequest,
    ) -> Result<ta_protocol::wire::PluginInspection, AppServiceError> {
        let package = PluginPackage::inspect(Path::new(&request.source_path))
            .map_err(|_| AppServiceError::PluginPackageInvalid)?;
        Ok(package.inspection().clone())
    }

    pub(crate) fn install_plugin_package(
        &self,
        owner_principal_id: &str,
        request: &InstallPluginPackageRequest,
        plugin_root: &Path,
    ) -> Result<InstallPluginPackageResult, AppServiceError> {
        let package = PluginPackage::inspect(Path::new(&request.source_path))
            .map_err(|_| AppServiceError::PluginPackageInvalid)?;
        if package.inspection() != &request.inspection {
            return Err(AppServiceError::PluginInspectionStale);
        }
        let granted = package
            .canonical_granted_capabilities(&request.granted_capabilities)
            .map_err(|_| AppServiceError::PluginCapabilityGrantInvalid)?;
        stage_exact_package(plugin_root, &package)?;
        let installation = PluginInstallation {
            plugin_id: package.inspection().plugin_id.clone(),
            version: package.inspection().version.clone(),
            digest_sha256: package.inspection().digest_sha256.clone(),
            requested_capabilities: package.inspection().requested_capabilities.clone(),
            granted_capabilities: granted,
            lifecycle_state: PluginLifecycleState::Disabled,
        };
        self.store
            .lock()
            .expect("app store should not be poisoned")
            .save_plugin_installation(owner_principal_id, installation.clone())?;
        Ok(InstallPluginPackageResult { installation })
    }

    pub(crate) fn list_plugin_installations(
        &self,
        owner_principal_id: &str,
    ) -> Result<ListPluginInstallationsResult, AppServiceError> {
        Ok(ListPluginInstallationsResult {
            installations: self
                .store
                .lock()
                .expect("app store should not be poisoned")
                .plugin_installations(owner_principal_id)?,
        })
    }

    pub(crate) fn uninstall_plugin(
        &self,
        owner_principal_id: &str,
        request: &UninstallPluginRequest,
    ) -> Result<(), AppServiceError> {
        let removed = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .remove_plugin_installation(
                owner_principal_id,
                &request.plugin_id,
                &request.version,
                &request.digest_sha256,
            )?;
        if !removed {
            return Err(AppServiceError::PluginInstallationNotFound);
        }
        Ok(())
    }
}

fn stage_exact_package(plugin_root: &Path, package: &PluginPackage) -> Result<(), AppServiceError> {
    fs::create_dir_all(plugin_root).map_err(|_| AppServiceError::PluginStageFailed)?;
    let target = plugin_root.join(&package.inspection().digest_sha256);
    if target.exists() {
        let existing =
            PluginPackage::inspect(&target).map_err(|_| AppServiceError::PluginStageFailed)?;
        if existing.inspection() != package.inspection() {
            return Err(AppServiceError::PluginStageFailed);
        }
        return Ok(());
    }
    let staged = plugin_root.join(format!(".stage-{}", Uuid::new_v4().simple()));
    fs::create_dir(&staged).map_err(|_| AppServiceError::PluginStageFailed)?;
    let write_result = (|| {
        fs::write(staged.join("manifest.json"), package.manifest_bytes())
            .map_err(|_| AppServiceError::PluginStageFailed)?;
        fs::write(
            staged.join(package.entrypoint_name()),
            package.entrypoint_bytes(),
        )
        .map_err(|_| AppServiceError::PluginStageFailed)?;
        let staged_package =
            PluginPackage::inspect(&staged).map_err(|_| AppServiceError::PluginStageFailed)?;
        if staged_package.inspection() != package.inspection() {
            return Err(AppServiceError::PluginStageFailed);
        }
        fs::rename(&staged, &target).map_err(|_| AppServiceError::PluginStageFailed)
    })();
    if write_result.is_err() && staged.exists() {
        let _ = fs::remove_dir_all(&staged);
    }
    write_result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ta_protocol::wire::{
        InspectPluginPackageRequest, InstallPluginPackageRequest, PluginCapability,
        PluginInstallation, PluginLifecycleState,
    };
    use ta_store::PluginRepository;

    use super::*;

    fn write_package(root: &Path, source: &str) {
        fs::create_dir_all(source).expect("source package directory");
        fs::write(
            Path::new(source).join("manifest.json"),
            r#"{"id":"example.plugin","version":"1.2.3","entrypoint":"plugin.js","capabilities":["workspaceRead"]}"#,
        )
        .expect("manifest");
        fs::write(Path::new(source).join("plugin.js"), "export default 1;").expect("entrypoint");
        fs::create_dir_all(root).expect("plugin root");
    }

    #[test]
    fn inspection_drift_is_rejected_before_staging_or_persistence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let plugin_root = temp.path().join("plugins");
        write_package(temp.path(), source.to_str().expect("utf8 source"));
        let service = AppService::bootstrap().expect("service");
        let request = InspectPluginPackageRequest {
            source_path: source.display().to_string(),
        };
        let inspection = service
            .inspect_plugin_package(&request)
            .expect("inspection");
        fs::write(source.join("plugin.js"), "export default 2;").expect("drift");

        let error = service
            .install_plugin_package(
                "principal-one",
                &InstallPluginPackageRequest {
                    source_path: source.display().to_string(),
                    inspection: inspection.clone(),
                    granted_capabilities: vec![PluginCapability::WorkspaceRead],
                },
                &plugin_root,
            )
            .expect_err("changed package must not install");
        assert!(matches!(error, AppServiceError::PluginInspectionStale));
        assert!(!plugin_root.join(&inspection.digest_sha256).exists());
        assert!(
            service
                .list_plugin_installations("principal-one")
                .expect("list")
                .installations
                .is_empty()
        );
    }

    #[test]
    fn exact_bytes_stage_before_duplicate_persistence_and_results_redact_source_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let plugin_root = temp.path().join("plugins");
        write_package(temp.path(), source.to_str().expect("utf8 source"));
        let service = AppService::bootstrap().expect("service");
        let source_path = source.display().to_string();
        let inspection = service
            .inspect_plugin_package(&InspectPluginPackageRequest {
                source_path: source_path.clone(),
            })
            .expect("inspection");
        let installation = PluginInstallation {
            plugin_id: inspection.plugin_id.clone(),
            version: inspection.version.clone(),
            digest_sha256: inspection.digest_sha256.clone(),
            requested_capabilities: inspection.requested_capabilities.clone(),
            granted_capabilities: vec![PluginCapability::WorkspaceRead],
            lifecycle_state: PluginLifecycleState::Disabled,
        };
        service
            .store
            .lock()
            .expect("store")
            .save_plugin_installation("principal-one", installation)
            .expect("preexisting durable row");

        assert!(
            service
                .install_plugin_package(
                    "principal-one",
                    &InstallPluginPackageRequest {
                        source_path: source_path.clone(),
                        inspection: inspection.clone(),
                        granted_capabilities: vec![PluginCapability::WorkspaceRead],
                    },
                    &plugin_root,
                )
                .is_err()
        );
        assert!(plugin_root.join(&inspection.digest_sha256).is_dir());

        let listed = service
            .list_plugin_installations("principal-one")
            .expect("list installations");
        assert!(
            !serde_json::to_string(&listed)
                .expect("serialize projection")
                .contains(&source_path)
        );
    }

    #[test]
    fn admission_delegates_grant_validation_and_preserves_explicit_empty_grants() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let plugin_root = temp.path().join("plugins");
        write_package(temp.path(), source.to_str().expect("utf8 source"));
        let service = AppService::bootstrap().expect("service");
        let source_path = source.display().to_string();
        let inspection = service
            .inspect_plugin_package(&InspectPluginPackageRequest {
                source_path: source_path.clone(),
            })
            .expect("inspection");

        for granted_capabilities in [
            vec![PluginCapability::WorkspaceWrite],
            vec![
                PluginCapability::WorkspaceRead,
                PluginCapability::WorkspaceRead,
            ],
        ] {
            assert!(matches!(
                service.install_plugin_package(
                    "principal-one",
                    &InstallPluginPackageRequest {
                        source_path: source_path.clone(),
                        inspection: inspection.clone(),
                        granted_capabilities,
                    },
                    &plugin_root,
                ),
                Err(AppServiceError::PluginCapabilityGrantInvalid)
            ));
        }

        let result = service
            .install_plugin_package(
                "principal-one",
                &InstallPluginPackageRequest {
                    source_path,
                    inspection,
                    granted_capabilities: vec![],
                },
                &plugin_root,
            )
            .expect("explicit empty grant installation");
        assert!(result.installation.granted_capabilities.is_empty());
    }
}
