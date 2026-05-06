use os_info::{Info, Version};

use crate::{HostCapabilities, HostOs, HostPlatform, LinuxDistribution, OsVersion, platform};

pub fn detect_current_platform() -> HostPlatform {
    let info = os_info::get();

    HostPlatform {
        os: HostOs::current(),
        version: OsVersion::from_os_info(info.version()),
        edition: info.edition().map(str::to_owned),
        linux_distribution: detect_linux_distribution(&info),
        capabilities: current_capabilities(),
    }
}

pub fn current_capabilities() -> HostCapabilities {
    platform::current_capabilities()
}

fn detect_linux_distribution(info: &Info) -> Option<LinuxDistribution> {
    if HostOs::current() != HostOs::Linux {
        return None;
    }

    Some(LinuxDistribution {
        id: info.os_type().to_string().to_lowercase(),
        name: info.os_type().to_string(),
        version: version_string(info.version()),
        edition: info.edition().map(str::to_owned),
    })
}

fn version_string(version: &Version) -> String {
    match version {
        Version::Unknown => "unknown".to_string(),
        other => other.to_string(),
    }
}
