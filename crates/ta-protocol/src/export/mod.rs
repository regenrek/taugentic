use std::path::Path;

use thiserror::Error;

use crate::wire::DAEMON_PROTOCOL_VERSION;

mod schema;
mod schema_core;
mod schema_core_runtime;
mod typescript;
mod typescript_generated;

pub use schema::export_json_schemas;
pub use typescript::export_typescript_bindings;

pub const PROTOCOL_VERSION: &str = DAEMON_PROTOCOL_VERSION;

#[derive(Debug, Error)]
pub enum ProtocolExportError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    TypeScript(#[from] ts_rs::ExportError),
}

pub fn export_protocol_artifacts(shared_package_dir: &Path) -> Result<(), ProtocolExportError> {
    export_typescript_bindings(shared_package_dir)?;
    export_json_schemas(shared_package_dir)?;
    Ok(())
}
