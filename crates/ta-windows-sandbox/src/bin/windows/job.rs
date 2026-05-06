use std::{ffi::c_void, mem, ptr};

use windows_sys::Win32::{
    Foundation::HANDLE,
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_UILIMIT_DESKTOP,
        JOB_OBJECT_UILIMIT_DISPLAYSETTINGS, JOB_OBJECT_UILIMIT_EXITWINDOWS,
        JOB_OBJECT_UILIMIT_GLOBALATOMS, JOB_OBJECT_UILIMIT_HANDLES,
        JOB_OBJECT_UILIMIT_READCLIPBOARD, JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS,
        JOB_OBJECT_UILIMIT_WRITECLIPBOARD, JOBOBJECT_BASIC_UI_RESTRICTIONS,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicUIRestrictions,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    },
};

use super::{HelperError, handle::Handle, util::win32_bool};

pub struct Job {
    handle: Handle,
}

impl Job {
    pub fn create_phase1() -> Result<Self, HelperError> {
        let handle = unsafe { CreateJobObjectW(ptr::null_mut(), ptr::null()) };
        let job = Self {
            handle: Handle::new(handle, "CreateJobObjectW")?,
        };
        job.apply_extended_limits()?;
        job.apply_ui_restrictions()?;
        Ok(job)
    }

    fn apply_extended_limits(&self) -> Result<(), HelperError> {
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        win32_bool(
            unsafe {
                SetInformationJobObject(
                    self.handle.raw(),
                    JobObjectExtendedLimitInformation,
                    &mut limits as *mut _ as *mut c_void,
                    mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            },
            "SetInformationJobObject(JobObjectExtendedLimitInformation)",
        )
    }

    fn apply_ui_restrictions(&self) -> Result<(), HelperError> {
        let mut restrictions = JOBOBJECT_BASIC_UI_RESTRICTIONS {
            UIRestrictionsClass: JOB_OBJECT_UILIMIT_HANDLES
                | JOB_OBJECT_UILIMIT_READCLIPBOARD
                | JOB_OBJECT_UILIMIT_WRITECLIPBOARD
                | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
                | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
                | JOB_OBJECT_UILIMIT_GLOBALATOMS
                | JOB_OBJECT_UILIMIT_DESKTOP
                | JOB_OBJECT_UILIMIT_EXITWINDOWS,
        };
        win32_bool(
            unsafe {
                SetInformationJobObject(
                    self.handle.raw(),
                    JobObjectBasicUIRestrictions,
                    &mut restrictions as *mut _ as *mut c_void,
                    mem::size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>() as u32,
                )
            },
            "SetInformationJobObject(JobObjectBasicUIRestrictions)",
        )
    }

    pub fn assign(&self, process: HANDLE) -> Result<(), HelperError> {
        win32_bool(
            unsafe { AssignProcessToJobObject(self.handle.raw(), process) },
            "AssignProcessToJobObject",
        )
    }
}
