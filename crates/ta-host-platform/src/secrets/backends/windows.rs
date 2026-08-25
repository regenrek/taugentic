use std::{ffi::c_void, ptr, slice};

use windows::{
    Win32::{
        Foundation::{ERROR_NO_SUCH_LOGON_SESSION, ERROR_NOT_FOUND},
        Security::Credentials::{
            CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
            CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
        },
    },
    core::{HRESULT, PCWSTR, PWSTR},
};

use crate::{HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue};

const BACKEND_NAME: &str = "windows-credential-manager";

pub(crate) struct WindowsCredentialManagerStore {
    service: String,
}

impl WindowsCredentialManagerStore {
    pub(crate) fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl HostSecretStore for WindowsCredentialManagerStore {
    fn store_secret(
        &self,
        key: &HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError> {
        let target_name = wide_null(&key.account_name(&self.service));
        let payload = value.expose_secret().as_bytes();
        let blob_size = u32::try_from(payload.len())
            .map_err(|error| HostSecretError::encrypt_failed(BACKEND_NAME, error))?;
        if blob_size > CRED_MAX_CREDENTIAL_BLOB_SIZE {
            return Err(HostSecretError::encrypt_failed(
                BACKEND_NAME,
                "secret payload exceeds Windows Credential Manager blob size limit",
            ));
        }
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_name.as_ptr().cast_mut()),
            CredentialBlobSize: blob_size,
            CredentialBlob: payload.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };

        unsafe { CredWriteW(&credential, 0) }
            .map_err(|error| HostSecretError::encrypt_failed(BACKEND_NAME, error))
    }

    fn load_secret(&self, key: &HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError> {
        let target_name = wide_null(&key.account_name(&self.service));
        let mut credential = ptr::null_mut();
        match unsafe {
            CredReadW(
                PCWSTR(target_name.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut credential,
            )
        } {
            Ok(()) => {
                let credential = CredentialPtr::new(credential);
                let payload = unsafe {
                    slice::from_raw_parts(
                        (*credential.as_ptr()).CredentialBlob,
                        (*credential.as_ptr()).CredentialBlobSize as usize,
                    )
                };
                String::from_utf8(payload.to_vec())
                    .map_err(|error| HostSecretError::decrypt_failed(BACKEND_NAME, error))
                    .and_then(HostSecretValue::new)
                    .map(Some)
            }
            Err(error) if is_not_found(&error) => Ok(None),
            Err(error) => Err(HostSecretError::decrypt_failed(BACKEND_NAME, error)),
        }
    }

    fn delete_secret(&self, key: &HostSecretKey) -> Result<(), HostSecretError> {
        let target_name = wide_null(&key.account_name(&self.service));
        match unsafe { CredDeleteW(PCWSTR(target_name.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if is_not_found(&error) => Ok(()),
            Err(error) => Err(HostSecretError::io_error("delete", error)),
        }
    }

    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }
}

struct CredentialPtr(*mut CREDENTIALW);

impl CredentialPtr {
    fn new(ptr: *mut CREDENTIALW) -> Self {
        Self(ptr)
    }

    fn as_ptr(&self) -> *mut CREDENTIALW {
        self.0
    }
}

impl Drop for CredentialPtr {
    fn drop(&mut self) {
        unsafe {
            CredFree(self.0.cast::<c_void>());
        }
    }
}

fn is_not_found(error: &windows::core::Error) -> bool {
    let code = error.code();
    code == HRESULT::from_win32(ERROR_NOT_FOUND.0)
        || code == HRESULT::from_win32(ERROR_NO_SUCH_LOGON_SESSION.0)
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
