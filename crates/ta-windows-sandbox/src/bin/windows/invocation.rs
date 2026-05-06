use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use ta_sandbox::{SandboxProfile, WINDOWS_SANDBOX_PROFILE_ARG};

use super::{HelperError, util::ARG_SEPARATOR};

#[derive(Debug)]
pub struct Invocation {
    pub profile: SandboxProfile,
    pub command: OsString,
    pub args: Vec<OsString>,
}

impl Invocation {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, HelperError> {
        let mut args = args.into_iter();
        let _program = args.next();
        let mut profile_json = None;

        while let Some(arg) = args.next() {
            if arg == OsStr::new(WINDOWS_SANDBOX_PROFILE_ARG) {
                profile_json = args.next();
                continue;
            }
            if arg == OsStr::new(ARG_SEPARATOR) {
                let command = args.next().ok_or(HelperError::MissingCommand)?;
                let args = args.collect();
                let profile_json = profile_json.ok_or(HelperError::MissingProfile)?;
                let profile = serde_json::from_slice(profile_json.as_encoded_bytes())?;
                return Ok(Self {
                    profile,
                    command,
                    args,
                });
            }
            return Err(HelperError::UnexpectedArg(arg));
        }

        Err(HelperError::MissingSeparator)
    }
}

#[derive(Debug)]
pub struct AbsoluteCommand(pub OsString);

impl AbsoluteCommand {
    pub fn new(command: OsString) -> Result<Self, HelperError> {
        if Path::new(&command).is_absolute() {
            Ok(Self(command))
        } else {
            Err(HelperError::RelativeCommand(command))
        }
    }
}
