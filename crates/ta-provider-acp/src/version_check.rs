use crate::descriptor::AcpLaunchKind;

pub const ACP_PROTOCOL_VERSION: u16 = 1;

pub const fn minimum_supported_protocol_version(launch_kind: AcpLaunchKind) -> Option<u16> {
    match launch_kind {
        AcpLaunchKind::Codex
        | AcpLaunchKind::Claude
        | AcpLaunchKind::Cursor
        | AcpLaunchKind::OpenCode
        | AcpLaunchKind::Copilot => Some(ACP_PROTOCOL_VERSION),
    }
}

pub const fn minimum_supported_cli_version(launch_kind: AcpLaunchKind) -> Option<&'static str> {
    match launch_kind {
        AcpLaunchKind::Codex
        | AcpLaunchKind::Claude
        | AcpLaunchKind::Cursor
        | AcpLaunchKind::OpenCode
        | AcpLaunchKind::Copilot => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::builtin_provider_descriptors;

    #[test]
    fn wave_one_flavors_require_acp_v1() {
        for provider in builtin_provider_descriptors() {
            assert_eq!(
                minimum_supported_protocol_version(provider.launch_kind()),
                Some(ACP_PROTOCOL_VERSION)
            );
        }
    }
}
