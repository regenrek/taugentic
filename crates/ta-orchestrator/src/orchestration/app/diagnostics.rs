use std::time::{SystemTime, UNIX_EPOCH};
use ta_host_platform::HostOs;

use ta_protocol::wire::{
    AgentRuntimeStrategyInfo, DaemonDiagnosticError, DaemonDiagnosticTokenUsage, DaemonDiagnostics,
    DaemonProviderHealthDiagnostic, DaemonSandboxCapabilitySnapshot, GetAgentRuntimeQuery,
    RunStatus,
};
use ta_store::PersistenceStore;

use super::{AppService, AppServiceError};
use crate::DaemonEvent;

const DIAGNOSTICS_EVENT_TAIL: usize = 4096;
const MAX_RECENT_DIAGNOSTIC_ERRORS: usize = 32;
const DIAGNOSTIC_MESSAGE_MAX_CHARS: usize = 280;
const RECENT_ERROR_TTL_MS: u64 = 24 * 60 * 60 * 1000;

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn diagnostics_snapshot(
        &self,
        host_platform: &ta_host_platform::HostPlatform,
        uptime_ms: u64,
        in_flight_rpc: u32,
    ) -> Result<DaemonDiagnostics, AppServiceError> {
        let agent_runtime = self.get_agent_runtime(&GetAgentRuntimeQuery {})?;
        let provider_health = agent_runtime
            .providers
            .iter()
            .map(provider_diagnostic)
            .collect::<Vec<_>>();

        let sandbox_probe = ta_host_platform::sandbox_capabilities();
        let sandbox = DaemonSandboxCapabilitySnapshot {
            os: host_os_label(host_platform.os),
            sandbox_kind: host_platform.capabilities.sandbox.to_string(),
            helper_available: sandbox_probe.helper_available,
            restricted_token_job: sandbox_probe.restricted_token_job,
            appcontainer: sandbox_probe.appcontainer,
            filesystem_allowlist: sandbox_probe.filesystem_allowlist,
            network_default_deny: sandbox_probe.network_default_deny,
            network_destination_allowlist: sandbox_probe.network_destination_allowlist,
        };

        let tail = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store.events_tail_desc(DIAGNOSTICS_EVENT_TAIL)?
        };

        let (token_usage, recent_errors) =
            scan_tail_for_tokens_and_errors(&tail, current_time_ms());

        let in_flight_capsule =
            u32::try_from(self.run_execution.active_run_count()).unwrap_or(u32::MAX);
        let worktree_count =
            u32::try_from(self.run_execution.workspace_run_count()).unwrap_or(u32::MAX);
        let claim_count = u32::try_from(self.run_execution.claim_count()).unwrap_or(u32::MAX);

        Ok(DaemonDiagnostics {
            uptime_ms,
            in_flight_rpc_count: in_flight_rpc,
            in_flight_capsule_run_count: in_flight_capsule,
            recent_error_count: recent_errors.len() as u32,
            recent_errors,
            token_usage,
            worktree_count,
            claim_count,
            sandbox,
            provider_health,
        })
    }
}

fn host_os_label(os: HostOs) -> String {
    match os {
        HostOs::Linux => "linux".to_string(),
        HostOs::Macos => "macos".to_string(),
        HostOs::Windows => "windows".to_string(),
    }
}

fn provider_diagnostic(provider: &AgentRuntimeStrategyInfo) -> DaemonProviderHealthDiagnostic {
    DaemonProviderHealthDiagnostic {
        provider_id: provider.id.as_str().to_string(),
        display_name: provider.display_name.clone(),
        status: provider.health.status,
        message: provider.health.message.clone(),
    }
}

fn scan_tail_for_tokens_and_errors(
    tail: &[ta_store::EventRecord],
    now_ms: u64,
) -> (DaemonDiagnosticTokenUsage, Vec<DaemonDiagnosticError>) {
    let mut token_usage = DaemonDiagnosticTokenUsage::default();
    let mut saw_real_usage = false;
    for record in tail.iter().rev() {
        if let DaemonEvent::TokenUsageRecorded(event) = &record.payload {
            token_usage.prompt_tokens = Some(
                token_usage
                    .prompt_tokens
                    .unwrap_or(0)
                    .saturating_add(event.prompt_tokens),
            );
            token_usage.completion_tokens = Some(
                token_usage
                    .completion_tokens
                    .unwrap_or(0)
                    .saturating_add(event.completion_tokens),
            );
            token_usage.cached_tokens = Some(
                token_usage
                    .cached_tokens
                    .unwrap_or(0)
                    .saturating_add(event.cached_tokens.unwrap_or(0)),
            );
            token_usage.reasoning_tokens = Some(
                token_usage
                    .reasoning_tokens
                    .unwrap_or(0)
                    .saturating_add(event.reasoning_tokens.unwrap_or(0)),
            );
            saw_real_usage = true;
        }
    }
    if saw_real_usage {
        token_usage.total_tokens = Some(
            token_usage.prompt_tokens.unwrap_or(0) + token_usage.completion_tokens.unwrap_or(0),
        );
    }

    let mut recent_errors = Vec::new();
    for record in tail.iter() {
        if recent_errors.len() >= MAX_RECENT_DIAGNOSTIC_ERRORS {
            break;
        }
        if now_ms.saturating_sub(record.occurred_at_ms) > RECENT_ERROR_TTL_MS {
            continue;
        }
        match &record.payload {
            DaemonEvent::Run(run)
                if matches!(run.status, RunStatus::Failed | RunStatus::BudgetExceeded) =>
            {
                recent_errors.push(DaemonDiagnosticError {
                    occurred_at_ms: record.occurred_at_ms,
                    source: "run".to_string(),
                    message: redact_diagnostic_text(&format!(
                        "{} · {}",
                        run.run_id.as_str(),
                        run.detail
                    )),
                });
            }
            DaemonEvent::RunReconciledOnStartup(event) => {
                recent_errors.push(DaemonDiagnosticError {
                    occurred_at_ms: record.occurred_at_ms,
                    source: "daemon.startup_reconciled".to_string(),
                    message: redact_diagnostic_text(&format!(
                        "{} · daemon restarted while run was active",
                        event.run_id.as_str()
                    )),
                });
            }
            _ => {}
        }
    }

    (token_usage, recent_errors)
}

fn redact_diagnostic_text(input: &str) -> String {
    let mut message = input.trim().to_string();
    if let Ok(home) = std::env::var("HOME")
        && home.len() > 1
    {
        message = message.replace(home.as_str(), "~");
    }
    if message.chars().count() > DIAGNOSTIC_MESSAGE_MAX_CHARS {
        let mut truncated = message
            .chars()
            .take(DIAGNOSTIC_MESSAGE_MAX_CHARS)
            .collect::<String>();
        truncated.push_str("...");
        message = truncated;
    }
    message
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}
