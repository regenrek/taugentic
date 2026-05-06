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

use crate::credential_store::{
    CredentialKey, CredentialStore, CredentialStoreError, StoredCredentials,
};

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

impl CredentialStore for WindowsCredentialManagerStore {
    fn store(
        &self,
        key: &CredentialKey,
        creds: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        let target_name = wide_null(&key.account_name(&self.service));
        let payload = serde_json::to_vec(creds)
            .map_err(|error| CredentialStoreError::serialization("encode", error))?;
        let blob_size = u32::try_from(payload.len())
            .map_err(|error| CredentialStoreError::serialization("encode", error))?;
        if blob_size > CRED_MAX_CREDENTIAL_BLOB_SIZE {
            return Err(CredentialStoreError::serialization(
                "encode",
                "credential payload exceeds Windows Credential Manager blob size limit",
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
            .map_err(|error| CredentialStoreError::encrypt_failed(BACKEND_NAME, error))
    }

    fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>, CredentialStoreError> {
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
                serde_json::from_slice(payload)
                    .map(Some)
                    .map_err(|error| CredentialStoreError::serialization("decode", error))
            }
            Err(error) if is_not_found(&error) => Ok(None),
            Err(error) => Err(CredentialStoreError::decrypt_failed(BACKEND_NAME, error)),
        }
    }

    fn delete(&self, key: &CredentialKey) -> Result<(), CredentialStoreError> {
        let target_name = wide_null(&key.account_name(&self.service));
        match unsafe { CredDeleteW(PCWSTR(target_name.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if is_not_found(&error) => Ok(()),
            Err(error) => Err(CredentialStoreError::io_error("delete", error)),
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
