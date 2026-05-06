use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use windows_sys::Win32::Foundation::GetLastError;

use super::HelperError;

pub const ARG_SEPARATOR: &str = "--";

pub fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

pub fn win32_bool(ok: i32, operation: &'static str) -> Result<(), HelperError> {
    if ok == 0 {
        Err(HelperError::Win32 {
            operation,
            code: last_error(),
        })
    } else {
        Ok(())
    }
}

pub fn win32_error(code: u32, operation: &'static str) -> Result<(), HelperError> {
    if code == 0 {
        Ok(())
    } else {
        Err(HelperError::Win32 { operation, code })
    }
}

pub fn hresult(ok: i32, operation: &'static str) -> Result<(), HelperError> {
    if ok < 0 {
        Err(HelperError::HResult {
            operation,
            code: ok,
        })
    } else {
        Ok(())
    }
}

pub fn last_error() -> u32 {
    unsafe { GetLastError() }
}
