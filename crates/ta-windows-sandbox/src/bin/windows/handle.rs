use std::ffi::c_void;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree, PSID};

use super::{HelperError, util::last_error};

pub struct Handle(HANDLE);

impl Handle {
    pub fn new(handle: HANDLE, operation: &'static str) -> Result<Self, HelperError> {
        if handle == 0 {
            Err(HelperError::Win32 {
                operation,
                code: last_error(),
            })
        } else {
            Ok(Self(handle))
        }
    }

    pub fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

pub struct Sid {
    ptr: PSID,
}

impl Sid {
    pub fn from_raw(ptr: PSID) -> Result<Self, HelperError> {
        if ptr.is_null() {
            Err(HelperError::Win32 {
                operation: "SID allocation",
                code: last_error(),
            })
        } else {
            Ok(Self { ptr })
        }
    }

    pub fn from_string(sid: &str) -> Result<Self, HelperError> {
        let wide = super::util::to_wide(sid);
        let mut ptr = std::ptr::null_mut::<c_void>();
        let ok = unsafe {
            windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW(
                wide.as_ptr(),
                &mut ptr,
            )
        };
        if ok == 0 || ptr.is_null() {
            Err(HelperError::Win32 {
                operation: "ConvertStringSidToSidW",
                code: last_error(),
            })
        } else {
            Ok(Self { ptr })
        }
    }

    pub fn raw(&self) -> PSID {
        self.ptr
    }
}

impl Drop for Sid {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                LocalFree(self.ptr);
            }
        }
    }
}
