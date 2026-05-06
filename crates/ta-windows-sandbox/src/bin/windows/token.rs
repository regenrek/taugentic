use std::ptr;

use windows_sys::Win32::{
    Foundation::HANDLE,
    Security::{
        CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, LUA_TOKEN, SID_AND_ATTRIBUTES,
        TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_SESSIONID, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE,
        TOKEN_QUERY,
    },
    System::Threading::GetCurrentProcess,
};

use super::{
    HelperError,
    handle::{Handle, Sid},
    util::last_error,
};

const PRIVILEGED_SIDS: &[&str] = &[
    "S-1-5-32-544", // Administrators
    "S-1-5-32-547", // Power Users
    "S-1-5-32-551", // Backup Operators
];

pub struct RestrictedToken {
    handle: Handle,
    _disabled_sids: Vec<Sid>,
}

impl RestrictedToken {
    pub fn create() -> Result<Self, HelperError> {
        let mut base_token = 0;
        let desired_access = TOKEN_DUPLICATE
            | TOKEN_ASSIGN_PRIMARY
            | TOKEN_QUERY
            | TOKEN_ADJUST_DEFAULT
            | TOKEN_ADJUST_SESSIONID;
        let ok =
            unsafe { open_process_token(GetCurrentProcess(), desired_access, &mut base_token) };
        let base_token = Handle::new(base_token, "OpenProcessToken").and_then(|handle| {
            if ok == 0 {
                Err(HelperError::Win32 {
                    operation: "OpenProcessToken",
                    code: last_error(),
                })
            } else {
                Ok(handle)
            }
        })?;

        let disabled_sids = PRIVILEGED_SIDS
            .iter()
            .map(|sid| Sid::from_string(sid))
            .collect::<Result<Vec<_>, _>>()?;
        let mut sid_attributes = disabled_sids
            .iter()
            .map(|sid| SID_AND_ATTRIBUTES {
                Sid: sid.raw(),
                Attributes: 0,
            })
            .collect::<Vec<_>>();
        let mut restricted = 0;
        let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN;
        let ok = unsafe {
            CreateRestrictedToken(
                base_token.raw(),
                flags,
                sid_attributes.len() as u32,
                sid_attributes.as_mut_ptr(),
                0,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                &mut restricted,
            )
        };
        if ok == 0 {
            return Err(HelperError::Win32 {
                operation: "CreateRestrictedToken",
                code: last_error(),
            });
        }

        Ok(Self {
            handle: Handle::new(restricted, "CreateRestrictedToken")?,
            _disabled_sids: disabled_sids,
        })
    }

    pub fn raw(&self) -> HANDLE {
        self.handle.raw()
    }
}

unsafe fn open_process_token(
    process_handle: HANDLE,
    desired_access: u32,
    token_handle: *mut HANDLE,
) -> i32 {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(
            ProcessHandle: HANDLE,
            DesiredAccess: u32,
            TokenHandle: *mut HANDLE,
        ) -> i32;
    }

    unsafe { OpenProcessToken(process_handle, desired_access, token_handle) }
}
