use std::ffi::OsString;

use appcontainer::AppContainerProfile;
use fs_allowlist::FilesystemAllowlist;
use invocation::{AbsoluteCommand, Invocation};
use job::Job;
use network_restrictions::NetworkRestrictions;
use process::Process;
use thiserror::Error;
use token::RestrictedToken;

mod appcontainer;
mod fs_allowlist;
mod handle;
mod invocation;
mod job;
mod network_restrictions;
mod process;
mod token;
mod util;
mod wfp_allowlist;

pub fn main() {
    match run(std::env::args_os()) {
        Ok(code) => std::process::exit(code as i32),
        Err(error) => {
            eprintln!("ta-windows-sandbox: {error}");
            std::process::exit(126)
        }
    }
}

fn run(args: impl IntoIterator<Item = OsString>) -> Result<u32, HelperError> {
    let invocation = Invocation::parse(args)?;
    ta_sandbox::windows::validate_windows_appcontainer_profile(&invocation.profile)
        .map_err(|error| HelperError::UnsupportedProfile(error.to_string()))?;
    let command = AbsoluteCommand::new(invocation.command)?;
    let mut appcontainer = AppContainerProfile::create(invocation.profile.network_policy())?;
    let _network_restrictions =
        NetworkRestrictions::apply(invocation.profile.network_policy(), appcontainer.sid())?;
    let _filesystem_allowlist =
        FilesystemAllowlist::apply(&invocation.profile, appcontainer.sid())?;
    let token = RestrictedToken::create()?;
    let job = Job::create_phase1()?;
    let process = Process::spawn_suspended(
        token.raw(),
        command,
        invocation.args,
        appcontainer.security_capabilities_mut(),
    )?;
    job.assign(process.process_handle())?;
    process.resume_and_wait()
}

#[derive(Debug, Error)]
enum HelperError {
    #[error("missing {}", ta_sandbox::WINDOWS_SANDBOX_PROFILE_ARG)]
    MissingProfile,
    #[error("missing command separator '--'")]
    MissingSeparator,
    #[error("missing command after '--'")]
    MissingCommand,
    #[error("unexpected argument {0:?}")]
    UnexpectedArg(OsString),
    #[error("sandbox command must be absolute: {0:?}")]
    RelativeCommand(OsString),
    #[error("{0}")]
    UnsupportedProfile(String),
    #[error("invalid profile JSON: {0}")]
    InvalidProfileJson(#[from] serde_json::Error),
    #[error("{operation} failed with Win32 error {code}")]
    Win32 { operation: &'static str, code: u32 },
    #[error("{operation} failed with HRESULT 0x{code:08x}")]
    HResult { operation: &'static str, code: i32 },
    #[error("WaitForSingleObject returned unexpected status {0}")]
    UnexpectedWaitStatus(u32),
}
