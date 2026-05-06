use std::{mem, ptr, time::SystemTime};

use ta_sandbox::{NetworkPolicy, windows::appcontainer_capability_names};
use windows_sys::Win32::{
    Foundation::PSID,
    Security::{
        Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile},
        SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES,
    },
};

use super::{
    HelperError,
    handle::Sid,
    util::{hresult, to_wide},
};

const INTERNET_CLIENT_CAPABILITY_SID: &str = "S-1-15-3-1";
const SE_GROUP_ENABLED: u32 = 4;

pub struct AppContainerProfile {
    name: Vec<u16>,
    sid: Sid,
    _capability_sids: Vec<Sid>,
    capability_attributes: Vec<SID_AND_ATTRIBUTES>,
    security_capabilities: SECURITY_CAPABILITIES,
}

impl AppContainerProfile {
    pub fn create(policy: &NetworkPolicy) -> Result<Self, HelperError> {
        let capability_sids = capability_sids(policy)?;
        let capability_attributes = capability_sids
            .iter()
            .map(|sid| SID_AND_ATTRIBUTES {
                Sid: sid.raw(),
                Attributes: SE_GROUP_ENABLED,
            })
            .collect::<Vec<_>>();
        let name = profile_name();
        let display = to_wide("Taugentic Sandbox");
        let description = to_wide("Taugentic temporary AppContainer sandbox");
        let mut sid = ptr::null_mut::<std::ffi::c_void>() as PSID;
        hresult(
            unsafe {
                CreateAppContainerProfile(
                    name.as_ptr(),
                    display.as_ptr(),
                    description.as_ptr(),
                    capability_attributes.as_ptr(),
                    capability_attributes.len() as u32,
                    &mut sid,
                )
            },
            "CreateAppContainerProfile",
        )?;
        let sid = Sid::from_raw(sid)?;
        let mut profile = Self {
            name,
            sid,
            _capability_sids: capability_sids,
            capability_attributes,
            security_capabilities: unsafe { mem::zeroed() },
        };
        profile.refresh_security_capabilities();
        Ok(profile)
    }

    pub fn security_capabilities_mut(&mut self) -> &mut SECURITY_CAPABILITIES {
        self.refresh_security_capabilities();
        &mut self.security_capabilities
    }

    pub fn sid(&self) -> PSID {
        self.sid.raw()
    }

    fn refresh_security_capabilities(&mut self) {
        self.security_capabilities.AppContainerSid = self.sid.raw();
        self.security_capabilities.Capabilities = self.capability_attributes.as_mut_ptr();
        self.security_capabilities.CapabilityCount = self.capability_attributes.len() as u32;
        self.security_capabilities.Reserved = 0;
    }
}

impl Drop for AppContainerProfile {
    fn drop(&mut self) {
        if !self.name.is_empty() {
            unsafe {
                DeleteAppContainerProfile(self.name.as_ptr());
            }
        }
    }
}

fn capability_sids(policy: &NetworkPolicy) -> Result<Vec<Sid>, HelperError> {
    appcontainer_capability_names(policy)
        .map_err(|error| HelperError::UnsupportedProfile(error.to_string()))?
        .iter()
        .map(|name| match *name {
            "internetClient" => Sid::from_string(INTERNET_CLIENT_CAPABILITY_SID),
            other => Err(HelperError::UnsupportedProfile(format!(
                "unsupported AppContainer capability {other}"
            ))),
        })
        .collect()
}

fn profile_name() -> Vec<u16> {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    to_wide(&format!(
        "Taugentic.Sandbox.{}.{}",
        std::process::id(),
        nanos
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_network_maps_to_internet_client_capability_sid() {
        let sids = capability_sids(&NetworkPolicy::Open).expect("capabilities");

        assert_eq!(sids.len(), 1);
    }

    #[test]
    fn default_deny_network_has_no_capability_sids() {
        let sids = capability_sids(&NetworkPolicy::Off).expect("capabilities");

        assert!(sids.is_empty());
    }
}
