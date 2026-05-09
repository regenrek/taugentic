use std::path::{Component, Path, PathBuf};

use ta_daemon_client::DaemonClient;
use ta_protocol::wire::{WorkspaceId, WorkspacePath};

use crate::{args::WorkspaceCommands, error::CliError, output::CommandOutput};

pub fn run(
    daemon_client: &DaemonClient,
    command: WorkspaceCommands,
) -> Result<Option<CommandOutput>, CliError> {
    let mut client = daemon_client.connect_persistent("ta-cli", env!("CARGO_PKG_VERSION"))?;
    match command {
        WorkspaceCommands::Open { path, trust } => {
            let path = resolve_workspace_path_input(path)?;
            let workspace = client.open_workspace(path, trust)?;
            Ok(Some(CommandOutput::WorkspaceOpen(workspace)))
        }
        WorkspaceCommands::List => {
            let workspaces = client.list_workspaces()?;
            Ok(Some(CommandOutput::WorkspaceList(workspaces)))
        }
        WorkspaceCommands::Get { id } => {
            let id = WorkspaceId::new(id)
                .map_err(|error| CliError::InvalidArgument(error.to_string()))?;
            let workspace = client.get_workspace(id)?;
            Ok(Some(CommandOutput::WorkspaceOpen(workspace)))
        }
    }
}

pub(crate) fn resolve_workspace_path_input(
    path: impl AsRef<Path>,
) -> Result<WorkspacePath, CliError> {
    let path = path.as_ref();
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| CliError::InvalidArgument(error.to_string()))?
            .join(path)
    };
    let normalized = normalize_absolute_path(&absolute)?;

    WorkspacePath::from_canonical_wire_value(normalized.to_string_lossy().into_owned())
        .map_err(|error| CliError::InvalidArgument(error.to_string()))
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf, CliError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(CliError::InvalidArgument(format!(
                        "workspace path escapes filesystem root: {}",
                        path.display()
                    )));
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }

    if normalized.is_absolute() {
        Ok(normalized)
    } else {
        Err(CliError::InvalidArgument(format!(
            "workspace path must resolve to an absolute path: {}",
            path.display()
        )))
    }
}
