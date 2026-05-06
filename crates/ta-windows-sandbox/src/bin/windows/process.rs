use std::{
    ffi::{OsStr, OsString, c_void},
    mem,
    os::windows::ffi::OsStrExt,
    ptr,
};

use windows_sys::Win32::{
    Foundation::HANDLE,
    Security::SECURITY_CAPABILITIES,
    System::{
        Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        Threading::{
            CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
            InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW,
            TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
};

use super::{
    HelperError,
    handle::Handle,
    invocation::AbsoluteCommand,
    util::{last_error, win32_bool},
};

const WAIT_OBJECT_0: u32 = 0;
const WAIT_FAILED: u32 = u32::MAX;
const INFINITE: u32 = u32::MAX;
const RESUME_THREAD_FAILED: u32 = u32::MAX;

pub struct Process {
    process: Handle,
    thread: Handle,
}

impl Process {
    pub fn spawn_suspended(
        token: HANDLE,
        command: AbsoluteCommand,
        args: Vec<OsString>,
        security_capabilities: &mut SECURITY_CAPABILITIES,
    ) -> Result<Self, HelperError> {
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push(command.0);
        argv.extend(args);
        let mut command_line = argv_to_command_line(&argv);
        let mut stdio_handles = [
            unsafe { GetStdHandle(STD_INPUT_HANDLE) },
            unsafe { GetStdHandle(STD_OUTPUT_HANDLE) },
            unsafe { GetStdHandle(STD_ERROR_HANDLE) },
        ];
        let attribute_list = ProcThreadAttributeList::for_appcontainer_launch(
            &mut stdio_handles,
            security_capabilities,
        )?;
        let mut startup_info: STARTUPINFOEXW = unsafe { mem::zeroed() };
        startup_info.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        startup_info.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup_info.StartupInfo.hStdInput = stdio_handles[0];
        startup_info.StartupInfo.hStdOutput = stdio_handles[1];
        startup_info.StartupInfo.hStdError = stdio_handles[2];
        startup_info.lpAttributeList = attribute_list.raw();
        let mut process_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
        let ok = unsafe {
            CreateProcessAsUserW(
                token,
                ptr::null(),
                command_line.as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                1,
                CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
                ptr::null_mut(),
                ptr::null(),
                &startup_info as *const STARTUPINFOEXW as *const STARTUPINFOW,
                &mut process_info,
            )
        };
        if ok == 0 {
            return Err(HelperError::Win32 {
                operation: "CreateProcessAsUserW",
                code: last_error(),
            });
        }

        Ok(Self {
            process: Handle::new(process_info.hProcess, "CreateProcessAsUserW(process)")?,
            thread: Handle::new(process_info.hThread, "CreateProcessAsUserW(thread)")?,
        })
    }

    pub fn process_handle(&self) -> HANDLE {
        self.process.raw()
    }

    pub fn resume_and_wait(self) -> Result<u32, HelperError> {
        let resume_result = unsafe { ResumeThread(self.thread.raw()) };
        if resume_result == RESUME_THREAD_FAILED {
            unsafe {
                TerminateProcess(self.process.raw(), 126);
            }
            return Err(HelperError::Win32 {
                operation: "ResumeThread",
                code: last_error(),
            });
        }

        let wait = unsafe { WaitForSingleObject(self.process.raw(), INFINITE) };
        if wait == WAIT_FAILED {
            return Err(HelperError::Win32 {
                operation: "WaitForSingleObject",
                code: last_error(),
            });
        }
        if wait != WAIT_OBJECT_0 {
            return Err(HelperError::UnexpectedWaitStatus(wait));
        }

        let mut exit_code = 0;
        win32_bool(
            unsafe { GetExitCodeProcess(self.process.raw(), &mut exit_code) },
            "GetExitCodeProcess",
        )?;
        Ok(exit_code)
    }
}

struct ProcThreadAttributeList {
    buffer: Vec<u8>,
}

impl ProcThreadAttributeList {
    fn for_appcontainer_launch(
        handles: &mut [HANDLE],
        security_capabilities: &mut SECURITY_CAPABILITIES,
    ) -> Result<Self, HelperError> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(HelperError::Win32 {
                operation: "InitializeProcThreadAttributeList(size)",
                code: last_error(),
            });
        }

        let mut buffer = vec![0; bytes];
        let list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        win32_bool(
            unsafe { InitializeProcThreadAttributeList(list, 2, 0, &mut bytes) },
            "InitializeProcThreadAttributeList",
        )?;
        let attribute_list = Self { buffer };
        attribute_list.update_handle_list(handles)?;
        attribute_list.update_security_capabilities(security_capabilities)?;
        Ok(attribute_list)
    }

    fn update_handle_list(&self, handles: &mut [HANDLE]) -> Result<(), HelperError> {
        win32_bool(
            unsafe {
                UpdateProcThreadAttribute(
                    self.raw(),
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    handles.as_mut_ptr() as *const c_void,
                    mem::size_of_val(handles),
                    ptr::null_mut(),
                    ptr::null(),
                )
            },
            "UpdateProcThreadAttribute(PROC_THREAD_ATTRIBUTE_HANDLE_LIST)",
        )
    }

    fn update_security_capabilities(
        &self,
        security_capabilities: &mut SECURITY_CAPABILITIES,
    ) -> Result<(), HelperError> {
        win32_bool(
            unsafe {
                UpdateProcThreadAttribute(
                    self.raw(),
                    0,
                    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                    security_capabilities as *mut _ as *const c_void,
                    mem::size_of::<SECURITY_CAPABILITIES>(),
                    ptr::null_mut(),
                    ptr::null(),
                )
            },
            "UpdateProcThreadAttribute(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES)",
        )
    }

    fn raw(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST
    }
}

impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.raw());
        }
    }
}

fn argv_to_command_line(argv: &[OsString]) -> Vec<u16> {
    let mut out = Vec::new();
    for (index, arg) in argv.iter().enumerate() {
        if index > 0 {
            out.push(' ' as u16);
        }
        quote_arg(arg.as_os_str(), &mut out);
    }
    out.push(0);
    out
}

fn quote_arg(arg: &OsStr, out: &mut Vec<u16>) {
    let encoded = arg.encode_wide().collect::<Vec<_>>();
    let needs_quotes = encoded.is_empty()
        || encoded
            .iter()
            .any(|ch| matches!(*ch, 0x20 | 0x09 | 0x0a | 0x0d | 0x22));
    if !needs_quotes {
        out.extend(encoded);
        return;
    }

    out.push('"' as u16);
    let mut backslashes = 0;
    for ch in encoded {
        match ch {
            0x5c => backslashes += 1,
            0x22 => {
                out.extend(std::iter::repeat_n(0x5c, backslashes * 2 + 1));
                out.push(0x22);
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n(0x5c, backslashes));
                backslashes = 0;
                out.push(ch);
            }
        }
    }
    out.extend(std::iter::repeat_n(0x5c, backslashes * 2));
    out.push('"' as u16);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_windows_arguments_for_create_process() {
        let args = vec![
            OsString::from(r"C:\Program Files\App\app.exe"),
            OsString::from(r#"say "hello""#),
            OsString::from(r"C:\tmp\"),
        ];

        let command_line = argv_to_command_line(&args);
        let command_line = String::from_utf16(&command_line[..command_line.len() - 1])
            .expect("utf16 command line");

        assert_eq!(
            command_line,
            r#""C:\Program Files\App\app.exe" "say \"hello\"" C:\tmp\"#
        );
    }
}
