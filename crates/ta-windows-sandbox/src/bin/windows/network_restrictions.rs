use std::{ptr, slice};

use ta_sandbox::{NetworkPolicy, windows::WindowsNetworkCapability};
use windows_sys::Win32::{
    Foundation::{LocalFree, PSID},
    NetworkManagement::WindowsFirewall::{
        NetworkIsolationGetAppContainerConfig, NetworkIsolationSetAppContainerConfig,
    },
    Security::{EqualSid, SID_AND_ATTRIBUTES},
};

use super::{HelperError, util::win32_error, wfp_allowlist::WfpAllowlist};

const SE_GROUP_ENABLED: u32 = 4;

pub struct NetworkRestrictions {
    loopback: Option<LoopbackExemption>,
    _allowlist: Option<WfpAllowlist>,
}

impl NetworkRestrictions {
    pub fn apply(policy: &NetworkPolicy, appcontainer_sid: PSID) -> Result<Self, HelperError> {
        let capability = ta_sandbox::windows::windows_network_capability(policy)
            .map_err(|error| HelperError::UnsupportedProfile(error.to_string()))?;
        let loopback = match capability {
            WindowsNetworkCapability::DefaultDeny | WindowsNetworkCapability::InternetClient => {
                None
            }
            WindowsNetworkCapability::DestinationAllowlist => None,
            WindowsNetworkCapability::Loopback => Some(LoopbackExemption::apply(appcontainer_sid)?),
        };
        let allowlist = match capability {
            WindowsNetworkCapability::DestinationAllowlist => {
                Some(WfpAllowlist::apply(policy, appcontainer_sid)?)
            }
            WindowsNetworkCapability::DefaultDeny
            | WindowsNetworkCapability::InternetClient
            | WindowsNetworkCapability::Loopback => None,
        };
        Ok(Self {
            loopback,
            _allowlist: allowlist,
        })
    }
}

impl Drop for NetworkRestrictions {
    fn drop(&mut self) {
        if let Some(loopback) = self.loopback.as_mut() {
            loopback.restore();
        }
    }
}

struct LoopbackExemption {
    original_count: u32,
    original_entries: *mut SID_AND_ATTRIBUTES,
    restored: bool,
}

impl LoopbackExemption {
    fn apply(appcontainer_sid: PSID) -> Result<Self, HelperError> {
        let mut original_count = 0;
        let mut original_entries = ptr::null_mut();
        win32_error(
            // SAFETY: Both out-pointers are valid for the duration of the call.
            // Windows owns the returned buffer until we release it with LocalFree.
            unsafe {
                NetworkIsolationGetAppContainerConfig(&mut original_count, &mut original_entries)
            },
            "NetworkIsolationGetAppContainerConfig",
        )?;

        let original = if original_entries.is_null() || original_count == 0 {
            &[][..]
        } else {
            // SAFETY: NetworkIsolationGetAppContainerConfig returned a buffer with
            // original_count entries, and we keep the buffer alive until restore.
            unsafe { slice::from_raw_parts(original_entries, original_count as usize) }
        };
        if contains_sid(original, appcontainer_sid) {
            // SAFETY: The buffer was allocated by the Windows API above and is no
            // longer needed because we did not mutate the exemption list.
            unsafe {
                LocalFree(original_entries.cast());
            }
            return Ok(Self {
                original_count: 0,
                original_entries: ptr::null_mut(),
                restored: true,
            });
        }

        let mut updated = original.to_vec();
        updated.push(SID_AND_ATTRIBUTES {
            Sid: appcontainer_sid,
            Attributes: SE_GROUP_ENABLED,
        });
        let set_result = win32_error(
            // SAFETY: updated points to SID_AND_ATTRIBUTES entries that remain
            // alive for the duration of the call; the API copies the config.
            unsafe {
                NetworkIsolationSetAppContainerConfig(updated.len() as u32, updated.as_ptr())
            },
            "NetworkIsolationSetAppContainerConfig(loopback)",
        );
        if let Err(error) = set_result {
            // SAFETY: original_entries came from NetworkIsolationGetAppContainerConfig.
            unsafe {
                LocalFree(original_entries.cast());
            }
            return Err(error);
        }

        Ok(Self {
            original_count,
            original_entries,
            restored: false,
        })
    }

    fn restore(&mut self) {
        if self.restored {
            return;
        }
        // SAFETY: original_entries is the still-live buffer returned by
        // NetworkIsolationGetAppContainerConfig; after restoring, we free it once.
        unsafe {
            NetworkIsolationSetAppContainerConfig(self.original_count, self.original_entries);
            LocalFree(self.original_entries.cast());
        }
        self.restored = true;
    }
}

impl Drop for LoopbackExemption {
    fn drop(&mut self) {
        self.restore();
    }
}

fn contains_sid(entries: &[SID_AND_ATTRIBUTES], sid: PSID) -> bool {
    entries
        .iter()
        // SAFETY: entry.Sid values come from Windows SID_AND_ATTRIBUTES entries,
        // and sid is the live AppContainer SID owned by AppContainerProfile.
        .any(|entry| unsafe { EqualSid(entry.Sid, sid) != 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_entries_do_not_contain_null_sid() {
        assert!(!contains_sid(&[], ptr::null_mut()));
    }
}
