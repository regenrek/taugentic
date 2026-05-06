use std::{ffi::c_void, mem, os::windows::ffi::OsStrExt, path::PathBuf, ptr};

use ta_sandbox::{
    SandboxProfile,
    windows::{WindowsFilesystemAccess, filesystem_grants},
};
use windows_sys::Win32::{
    Foundation::{LocalFree, PSID},
    Security::{
        ACL,
        Authorization::{
            BuildTrusteeWithSidW, EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW,
            SE_FILE_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_W,
        },
        DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
    },
    Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE},
};

use super::{HelperError, util::win32_error};

pub struct FilesystemAllowlist {
    restorations: Vec<DaclRestoration>,
}

impl FilesystemAllowlist {
    pub fn apply(profile: &SandboxProfile, appcontainer_sid: PSID) -> Result<Self, HelperError> {
        let mut restorations = Vec::new();
        for grant in filesystem_grants(profile) {
            restorations.push(DaclRestoration::grant(
                canonical_path(grant.path())?,
                appcontainer_sid,
                access_mask(grant.access()),
            )?);
        }
        Ok(Self { restorations })
    }
}

impl Drop for FilesystemAllowlist {
    fn drop(&mut self) {
        for restoration in self.restorations.iter_mut().rev() {
            restoration.restore();
        }
    }
}

struct DaclRestoration {
    path: Vec<u16>,
    original_dacl: *mut ACL,
    security_descriptor: PSECURITY_DESCRIPTOR,
    restored: bool,
}

impl DaclRestoration {
    fn grant(
        path: Vec<u16>,
        appcontainer_sid: PSID,
        access_mask: u32,
    ) -> Result<Self, HelperError> {
        let mut original_dacl = ptr::null_mut();
        let mut security_descriptor = ptr::null_mut();
        win32_error(
            unsafe {
                GetNamedSecurityInfoW(
                    path.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut original_dacl,
                    ptr::null_mut(),
                    &mut security_descriptor,
                )
            },
            "GetNamedSecurityInfoW",
        )?;

        let mut trustee: TRUSTEE_W = unsafe { mem::zeroed() };
        unsafe {
            BuildTrusteeWithSidW(&mut trustee, appcontainer_sid);
        }
        let access = EXPLICIT_ACCESS_W {
            grfAccessPermissions: access_mask,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
            Trustee: trustee,
        };
        let mut new_dacl = ptr::null_mut();
        win32_error(
            unsafe { SetEntriesInAclW(1, &access, original_dacl, &mut new_dacl) },
            "SetEntriesInAclW",
        )?;
        let set_result = win32_error(
            unsafe {
                SetNamedSecurityInfoW(
                    path.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    new_dacl,
                    ptr::null_mut(),
                )
            },
            "SetNamedSecurityInfoW(grant)",
        );
        unsafe {
            LocalFree(new_dacl as *mut c_void);
        }
        set_result?;

        Ok(Self {
            path,
            original_dacl,
            security_descriptor,
            restored: false,
        })
    }

    fn restore(&mut self) {
        if self.restored {
            return;
        }
        unsafe {
            SetNamedSecurityInfoW(
                self.path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                self.original_dacl,
                ptr::null_mut(),
            );
            LocalFree(self.security_descriptor);
        }
        self.restored = true;
    }
}

impl Drop for DaclRestoration {
    fn drop(&mut self) {
        self.restore();
    }
}

fn canonical_path(path: &PathBuf) -> Result<Vec<u16>, HelperError> {
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        HelperError::UnsupportedProfile(format!(
            "filesystem allowlist path is not accessible: {error}"
        ))
    })?;
    Ok(canonical.as_os_str().encode_wide().chain(Some(0)).collect())
}

fn access_mask(access: WindowsFilesystemAccess) -> u32 {
    match access {
        WindowsFilesystemAccess::Read => FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
        WindowsFilesystemAccess::Write => {
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_and_write_access_masks_are_distinct() {
        assert_ne!(
            access_mask(WindowsFilesystemAccess::Read),
            access_mask(WindowsFilesystemAccess::Write)
        );
    }
}
