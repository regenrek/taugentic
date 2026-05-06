use std::{path::PathBuf, time::Duration};

use ta_exec::SandboxProfile;

use crate::{mcp::AcpMcpServerSpec, mode_mapping::ModeMapping};

pub const DEFAULT_CANCEL_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpProcessConfig {
    pub flavor_id: String,
    pub command: PathBuf,
    pub sandbox_profile: SandboxProfile,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub env_remove: Vec<String>,
    pub work_dir: PathBuf,
    pub mcp_servers: Vec<AcpMcpServerSpec>,
    pub session_mode_id: Option<String>,
    pub session_model_id: Option<String>,
    pub mode_mapping: ModeMapping,
    pub cancel_grace: Duration,
}
