//! Linux sandbox helper profile semantics.
//!
//! `network: Off` installs filesystem Landlock plus TCP network denial through
//! Landlock and seccomp. If Landlock is unavailable, the helper falls back to
//! bubblewrap for `network: Off` only. `network: Open` installs only filesystem
//! Landlock. `network: Allowlist` installs Landlock TCP connect port rules.
//! `network: Loopback` remains fail-closed until Linux has address-aware
//! enforcement here.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    linux::main()
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("ta-linux-sandbox: Linux Landlock sandbox helper only runs on Linux");
    std::process::ExitCode::from(126)
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        collections::{BTreeMap, BTreeSet},
        env,
        ffi::{OsStr, OsString},
        fs,
        os::fd::{OwnedFd, RawFd},
        os::unix::process::CommandExt,
        path::{Path, PathBuf},
        process::{Command, ExitCode},
    };

    use landlock::{
        ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, NetPort, RestrictionStatus,
        Ruleset, RulesetAttr, RulesetCreated, RulesetCreatedAttr, RulesetStatus,
    };
    use nix::{
        errno::Errno,
        sys::resource::{Resource, getrlimit},
        unistd,
    };
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule, TargetArch,
    };
    use ta_host_platform::is_safe_bwrap_binary;
    use ta_sandbox::{LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG, NetworkPolicy, SandboxProfile};
    use thiserror::Error;

    const PROFILE_ARG: &str = "--profile-json";
    const BWRAP_CHILD_ARG: &str = "--bwrap-child";
    const ARG_SEPARATOR: &str = "--";
    const BWRAP_PROGRAM: &str = "/usr/bin/bwrap";
    const BWRAP_NETWORK_OFF_ONLY: &str =
        "bwrap fallback supports only network: Off; use Linux 6.7+ for Landlock TCP rules";
    const LOOPBACK_REQUIRES_ADDRESS_AWARE_BACKEND: &str = "network: Loopback requires address-aware Linux enforcement; Landlock TCP rules are port-only";
    const BWRAP_BASE_ENV_ALLOWLIST: &[&str] = &["PATH", "HOME", "LANG", "TMPDIR"];
    const BWRAP_PROCESS_PATH: &str = "/usr/bin:/bin";
    const LANDLOCK_FS_ABI: ABI = ABI::V1;
    const LANDLOCK_NETWORK_ABI: ABI = ABI::V4;
    const FIRST_INHERITED_FD: RawFd = 3;
    const PLATFORM_READ_PATHS: &[&str] = &[
        "/bin",
        "/sbin",
        "/usr",
        "/etc",
        "/lib",
        "/lib64",
        "/nix/store",
        "/run/current-system/sw",
        "/proc",
        "/dev/null",
        "/dev/urandom",
    ];
    const BWRAP_PLATFORM_RO_PATHS: &[&str] = &[
        "/bin",
        "/sbin",
        "/usr",
        "/etc",
        "/lib",
        "/lib64",
        "/nix/store",
        "/run/current-system/sw",
    ];

    pub fn main() -> ExitCode {
        match run(env::args_os()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("ta-linux-sandbox: {error}");
                ExitCode::from(126)
            }
        }
    }

    /// Execution order is parse/validate, inherited-FD hygiene, sandbox selection/install, then exec.
    fn run(args: impl IntoIterator<Item = OsString>) -> Result<(), HelperError> {
        run_with_hooks(
            args,
            RunHooks {
                close_inherited_fds,
                capability_probe: landlock_capability_available,
                apply_profile,
                exec_bwrap_fallback,
                exec_command,
            },
        )
    }

    fn run_with_hooks<C, P, A, B, E>(
        args: impl IntoIterator<Item = OsString>,
        hooks: RunHooks<C, P, A, B, E>,
    ) -> Result<(), HelperError>
    where
        C: FnOnce() -> Result<(), HelperError>,
        P: for<'network> FnOnce(&'network LandlockNetwork) -> Result<bool, HelperError>,
        A: for<'profile, 'network> FnOnce(
            &'profile SandboxProfile,
            &'network LandlockNetwork,
        ) -> Result<(), HelperError>,
        B: for<'invocation> FnOnce(&'invocation Invocation) -> Result<(), HelperError>,
        E: FnOnce(AbsoluteCommand, Vec<OsString>) -> Result<(), HelperError>,
    {
        let RunHooks {
            close_inherited_fds,
            capability_probe,
            apply_profile,
            exec_bwrap_fallback,
            exec_command,
        } = hooks;
        let invocation = Invocation::parse(args)?;
        let command = AbsoluteCommand::new(invocation.command.clone())?;
        close_inherited_fds()?;
        if invocation.bwrap_child {
            validate_bwrap_child_profile(&invocation.profile)?;
        } else {
            match decide_sandbox_strategy(&invocation.profile, capability_probe)? {
                SandboxStrategy::Landlock(network) => apply_profile(&invocation.profile, &network)?,
                SandboxStrategy::Bwrap => return exec_bwrap_fallback(&invocation),
            }
        }
        exec_command(command, invocation.args)
    }

    struct RunHooks<C, P, A, B, E> {
        close_inherited_fds: C,
        capability_probe: P,
        apply_profile: A,
        exec_bwrap_fallback: B,
        exec_command: E,
    }

    #[derive(Debug)]
    struct Invocation {
        profile_json: OsString,
        profile: SandboxProfile,
        command: OsString,
        args: Vec<OsString>,
        bwrap_child: bool,
        caller_env_present: bool,
    }

    impl Invocation {
        fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, HelperError> {
            let mut args = args.into_iter();
            let _program = args.next();
            let mut profile_json = None;
            let mut bwrap_child = false;
            let mut caller_env_present = false;

            while let Some(arg) = args.next() {
                if arg == OsStr::new(BWRAP_CHILD_ARG) {
                    bwrap_child = true;
                    continue;
                }
                if arg == OsStr::new(LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG) {
                    caller_env_present = true;
                    continue;
                }
                if arg == OsStr::new(PROFILE_ARG) {
                    profile_json = args.next();
                    continue;
                }
                if arg == OsStr::new(ARG_SEPARATOR) {
                    let command = args.next().ok_or(HelperError::MissingCommand)?;
                    let args = args.collect();
                    let profile_json = profile_json.ok_or(HelperError::MissingProfile)?;
                    let profile = serde_json::from_slice(profile_json.as_encoded_bytes())?;
                    return Ok(Self {
                        profile_json,
                        profile,
                        command,
                        args,
                        bwrap_child,
                        caller_env_present,
                    });
                }
                return Err(HelperError::UnexpectedArg(arg));
            }

            Err(HelperError::MissingSeparator)
        }
    }

    #[derive(Debug)]
    struct AbsoluteCommand(OsString);

    impl AbsoluteCommand {
        fn new(command: OsString) -> Result<Self, HelperError> {
            if Path::new(&command).is_absolute() {
                Ok(Self(command))
            } else {
                Err(HelperError::RelativeCommand(command))
            }
        }
    }

    fn landlock_network(profile: &SandboxProfile) -> Result<LandlockNetwork, HelperError> {
        Ok(match profile.network_policy() {
            NetworkPolicy::Off => LandlockNetwork::DenyTcp,
            NetworkPolicy::Open => LandlockNetwork::Open,
            NetworkPolicy::Loopback => {
                return Err(HelperError::UnsupportedProfile(
                    LOOPBACK_REQUIRES_ADDRESS_AWARE_BACKEND,
                ));
            }
            NetworkPolicy::Allowlist(entries) => {
                LandlockNetwork::AllowTcpConnect(parse_tcp_allowlist(entries)?)
            }
        })
    }

    fn decide_sandbox_strategy(
        profile: &SandboxProfile,
        capability_probe: impl for<'network> FnOnce(
            &'network LandlockNetwork,
        ) -> Result<bool, HelperError>,
    ) -> Result<SandboxStrategy, HelperError> {
        let network = landlock_network(profile)?;
        if capability_probe(&network)? {
            return Ok(SandboxStrategy::Landlock(network));
        }

        validate_bwrap_child_profile(profile)?;
        Ok(SandboxStrategy::Bwrap)
    }

    fn apply_profile(
        profile: &SandboxProfile,
        network: &LandlockNetwork,
    ) -> Result<(), HelperError> {
        let status = install_landlock(profile, network)?;
        if status.ruleset == RulesetStatus::NotEnforced {
            return Err(HelperError::LandlockUnavailable(
                network.unavailable_message(),
            ));
        }

        if matches!(network, LandlockNetwork::DenyTcp) {
            install_no_network_seccomp()?;
        }

        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum LandlockNetwork {
        Open,
        DenyTcp,
        AllowTcpConnect(Vec<u16>),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum SandboxStrategy {
        Landlock(LandlockNetwork),
        Bwrap,
    }

    impl LandlockNetwork {
        fn abi(&self) -> ABI {
            match self {
                Self::Open => LANDLOCK_FS_ABI,
                Self::DenyTcp | Self::AllowTcpConnect(_) => LANDLOCK_NETWORK_ABI,
            }
        }

        fn handles_tcp(&self) -> bool {
            matches!(self, Self::DenyTcp | Self::AllowTcpConnect(_))
        }

        fn tcp_connect_ports(&self) -> &[u16] {
            match self {
                Self::AllowTcpConnect(ports) => ports,
                Self::Open | Self::DenyTcp => &[],
            }
        }

        fn unavailable_message(&self) -> &'static str {
            match self {
                Self::Open => {
                    "Landlock ruleset was not enforced; Linux 5.13+ is required for filesystem rules"
                }
                Self::DenyTcp => {
                    "Landlock ruleset was not enforced; Linux 5.13+ is required for filesystem rules and Linux 6.7+ Landlock ABI v4 is required for TCP network denial"
                }
                Self::AllowTcpConnect(_) => {
                    "Landlock ruleset was not enforced; Linux 5.13+ is required for filesystem rules and Linux 6.7+ Landlock ABI v4 is required for TCP port allowlists"
                }
            }
        }
    }

    fn parse_tcp_allowlist(entries: &[String]) -> Result<Vec<u16>, HelperError> {
        let mut ports = Vec::with_capacity(entries.len());
        for entry in entries {
            ports.push(parse_tcp_port(entry)?);
        }
        ports.sort_unstable();
        ports.dedup();
        Ok(ports)
    }

    fn parse_tcp_port(entry: &str) -> Result<u16, HelperError> {
        let port = entry.strip_prefix("tcp:").unwrap_or(entry);
        let port = port.parse::<u16>().map_err(|_| {
            HelperError::UnsupportedProfile(
                "network: Allowlist currently supports only TCP port entries such as \"443\" or \"tcp:443\"; hostname/IP allowlists are a follow-up",
            )
        })?;
        if port == 0 {
            return Err(HelperError::UnsupportedProfile(
                "network: Allowlist TCP port 0 is ambiguous in Landlock and is not supported",
            ));
        }
        Ok(port)
    }

    fn landlock_capability_available(network: &LandlockNetwork) -> Result<bool, HelperError> {
        let abi = network.abi();
        let access_all = AccessFs::from_all(abi);
        let Ok(mut ruleset) = Ruleset::default()
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(access_all)
        else {
            return Ok(false);
        };
        if network.handles_tcp() {
            let Ok(network_ruleset) =
                ruleset.handle_access(AccessNet::BindTcp | AccessNet::ConnectTcp)
            else {
                return Ok(false);
            };
            ruleset = network_ruleset;
        }

        match ruleset.create() {
            Ok(created) => {
                let fd: Option<OwnedFd> = created.into();
                Ok(fd.is_some())
            }
            Err(_) => Ok(false),
        }
    }

    fn install_landlock(
        profile: &SandboxProfile,
        network: &LandlockNetwork,
    ) -> Result<RestrictionStatus, HelperError> {
        let abi = network.abi();
        let access_all = AccessFs::from_all(abi);
        let access_read = AccessFs::from_read(abi);
        let read_paths = PLATFORM_READ_PATHS
            .iter()
            .map(Path::new)
            .chain(profile.fs_read_paths().iter().map(|path| path.as_path()));

        let mut ruleset = Ruleset::default()
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(access_all)?;
        if network.handles_tcp() {
            ruleset = ruleset.handle_access(AccessNet::BindTcp | AccessNet::ConnectTcp)?;
        }

        let mut ruleset = ruleset
            .create()?
            .add_rules(landlock::path_beneath_rules(read_paths, access_read))?
            .add_rules(landlock::path_beneath_rules(
                profile.fs_write_paths(),
                access_all,
            ))?;
        ruleset = add_tcp_connect_port_rules(ruleset, network.tcp_connect_ports())?;
        let ruleset = ruleset.set_no_new_privs(true);

        Ok(ruleset.restrict_self()?)
    }

    fn add_tcp_connect_port_rules(
        ruleset: RulesetCreated,
        ports: &[u16],
    ) -> Result<RulesetCreated, HelperError> {
        Ok(ruleset.add_rules(ports.iter().copied().map(|port| {
            Ok::<_, landlock::RulesetError>(NetPort::new(port, AccessNet::ConnectTcp))
        }))?)
    }

    fn exec_bwrap_fallback(invocation: &Invocation) -> Result<(), HelperError> {
        validate_bwrap_child_profile(&invocation.profile)?;
        let bwrap = bwrap_program()?;
        let helper = env::current_exe().map_err(HelperError::CurrentExe)?;
        let args = build_bwrap_args(invocation, helper)?;
        let source = exec_bwrap_process(bwrap, args);
        Err(HelperError::BwrapExec { source })
    }

    fn exec_bwrap_process(bwrap: &Path, args: Vec<OsString>) -> std::io::Error {
        Command::new(bwrap)
            .env_clear()
            .env("PATH", BWRAP_PROCESS_PATH)
            .args(args)
            .exec()
    }

    fn validate_bwrap_child_profile(profile: &SandboxProfile) -> Result<(), HelperError> {
        if matches!(profile.network_policy(), NetworkPolicy::Off) {
            Ok(())
        } else {
            Err(HelperError::UnsupportedProfile(BWRAP_NETWORK_OFF_ONLY))
        }
    }

    fn bwrap_program() -> Result<&'static Path, HelperError> {
        let path = Path::new(BWRAP_PROGRAM);
        if is_safe_bwrap_binary(path) {
            Ok(path)
        } else {
            Err(HelperError::BwrapMissing)
        }
    }

    fn build_bwrap_args(
        invocation: &Invocation,
        helper: PathBuf,
    ) -> Result<Vec<OsString>, HelperError> {
        build_bwrap_args_with_env(invocation, helper, |name| env::var_os(name))
    }

    fn build_bwrap_args_with_env(
        invocation: &Invocation,
        helper: PathBuf,
        env_var: impl FnMut(&str) -> Option<OsString>,
    ) -> Result<Vec<OsString>, HelperError> {
        if invocation.caller_env_present {
            return Err(HelperError::BwrapCallerEnvUnsupported);
        }
        let mut args = vec![
            OsString::from("--unshare-user"),
            OsString::from("--unshare-net"),
            OsString::from("--die-with-parent"),
            OsString::from("--clearenv"),
        ];
        append_allowed_base_env(&mut args, &invocation.profile, env_var);
        append_bwrap_binds(&mut args, &invocation.profile, &helper)?;
        args.extend([
            OsString::from("--proc"),
            OsString::from("/proc"),
            OsString::from("--dev"),
            OsString::from("/dev"),
            helper.into_os_string(),
            OsString::from(BWRAP_CHILD_ARG),
            OsString::from(PROFILE_ARG),
            invocation.profile_json.clone(),
            OsString::from(ARG_SEPARATOR),
            invocation.command.clone(),
        ]);
        args.extend(invocation.args.iter().cloned());
        Ok(args)
    }

    fn append_allowed_base_env(
        args: &mut Vec<OsString>,
        profile: &SandboxProfile,
        mut env_var: impl FnMut(&str) -> Option<OsString>,
    ) {
        // bwrap accepts env only via argv --setenv. Never forward caller-provided
        // env values here; only rehydrate non-secret base env from the helper's parent.
        for name in profile.env_allowlist() {
            if BWRAP_BASE_ENV_ALLOWLIST.contains(&name.as_str())
                && let Some(value) = env_var(name)
            {
                args.extend([OsString::from("--setenv"), OsString::from(name), value]);
            }
        }
    }

    fn append_bwrap_binds(
        args: &mut Vec<OsString>,
        profile: &SandboxProfile,
        helper: &Path,
    ) -> Result<(), HelperError> {
        let mut writable_paths = BTreeSet::new();
        for path in profile.fs_write_paths() {
            validate_absolute_bwrap_path(path)?;
            writable_paths.insert(path.clone());
        }

        let mut read_paths = BTreeSet::new();
        for path in BWRAP_PLATFORM_RO_PATHS.iter().map(Path::new) {
            if path.exists() {
                read_paths.insert(path.to_path_buf());
            }
        }
        read_paths.insert(helper.to_path_buf());
        for path in profile.fs_read_paths() {
            validate_absolute_bwrap_path(path)?;
            read_paths.insert(path.clone());
        }

        for path in read_paths {
            if !writable_paths.contains(&path) {
                append_bwrap_bind(args, "--ro-bind", &path);
            }
        }
        for path in writable_paths {
            append_bwrap_bind(args, "--bind", &path);
        }
        Ok(())
    }

    fn validate_absolute_bwrap_path(path: &Path) -> Result<(), HelperError> {
        if path.is_absolute() {
            Ok(())
        } else {
            Err(HelperError::RelativeBwrapPath(path.to_path_buf()))
        }
    }

    fn append_bwrap_bind(args: &mut Vec<OsString>, option: &str, path: &Path) {
        args.extend([
            OsString::from(option),
            path.as_os_str().to_owned(),
            path.as_os_str().to_owned(),
        ]);
    }

    fn install_no_network_seccomp() -> Result<(), HelperError> {
        let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
        for syscall in [
            libc::SYS_accept,
            libc::SYS_accept4,
            libc::SYS_bind,
            libc::SYS_connect,
            libc::SYS_getpeername,
            libc::SYS_getsockname,
            libc::SYS_listen,
            libc::SYS_recvmmsg,
            libc::SYS_sendmmsg,
            libc::SYS_sendto,
            libc::SYS_setsockopt,
            libc::SYS_shutdown,
        ] {
            rules.insert(syscall, Vec::new());
        }

        let unix_only_rule = SeccompRule::new(vec![SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Ne,
            libc::AF_UNIX as u64,
        )?])?;
        rules.insert(libc::SYS_socket, vec![unix_only_rule.clone()]);
        rules.insert(libc::SYS_socketpair, vec![unix_only_rule]);

        let filter: BpfProgram = SeccompFilter::new(
            rules,
            SeccompAction::Allow,
            SeccompAction::Errno(libc::EPERM as u32),
            target_arch()?,
        )?
        .try_into()?;
        seccompiler::apply_filter(&filter)?;
        Ok(())
    }

    fn target_arch() -> Result<TargetArch, HelperError> {
        std::env::consts::ARCH
            .try_into()
            .map_err(|_| HelperError::UnsupportedArchitecture(std::env::consts::ARCH))
    }

    fn exec_command(command: AbsoluteCommand, args: Vec<OsString>) -> Result<(), HelperError> {
        let command = command.0;
        let source = Command::new(&command).args(args).exec();
        Err(HelperError::Exec { command, source })
    }

    fn close_inherited_fds() -> Result<(), HelperError> {
        close_inherited_fds_with(fs::read_dir("/proc/self/fd"), close_fds_to_rlimit)
    }

    fn close_inherited_fds_with<I, F>(
        proc_fds: Result<I, std::io::Error>,
        fallback_to_rlimit: F,
    ) -> Result<(), HelperError>
    where
        I: IntoIterator<Item = Result<fs::DirEntry, std::io::Error>>,
        F: FnOnce() -> Result<(), HelperError>,
    {
        match proc_fds {
            Ok(entries) => close_fds_from_proc(entries).map_err(HelperError::FdScan),
            Err(error) if should_fallback_to_rlimit(&error) => fallback_to_rlimit(),
            Err(error) => Err(HelperError::FdScan(error)),
        }
    }

    fn should_fallback_to_rlimit(error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
        )
    }

    fn close_fds_from_proc(
        entries: impl IntoIterator<Item = Result<fs::DirEntry, std::io::Error>>,
    ) -> Result<(), std::io::Error> {
        let mut fds = Vec::new();
        for entry in entries {
            if let Some(fd) = parse_fd_entry(entry?) {
                fds.push(fd);
            }
        }

        for fd in fds {
            close_fd(fd).map_err(std::io::Error::other)?;
        }
        Ok(())
    }

    fn parse_fd_entry(entry: fs::DirEntry) -> Option<RawFd> {
        let fd = entry.file_name().to_str()?.parse::<RawFd>().ok()?;
        (fd >= FIRST_INHERITED_FD).then_some(fd)
    }

    fn close_fds_to_rlimit() -> Result<(), HelperError> {
        let (soft_limit, _) = getrlimit(Resource::RLIMIT_NOFILE).map_err(HelperError::FdLimit)?;
        let max_fd = soft_limit.min(i32::MAX as _);
        for fd in FIRST_INHERITED_FD..max_fd as RawFd {
            close_fd(fd)?;
        }
        Ok(())
    }

    fn close_fd(fd: RawFd) -> Result<(), HelperError> {
        match unistd::close(fd) {
            Ok(()) | Err(Errno::EBADF) => Ok(()),
            Err(error) => Err(HelperError::FdClose { fd, error }),
        }
    }

    #[derive(Debug, Error)]
    enum HelperError {
        #[error("missing {PROFILE_ARG} argument")]
        MissingProfile,
        #[error("missing {ARG_SEPARATOR} before command")]
        MissingSeparator,
        #[error("missing command after {ARG_SEPARATOR}")]
        MissingCommand,
        #[error("unexpected argument before {ARG_SEPARATOR}: {0:?}")]
        UnexpectedArg(OsString),
        #[error("invalid sandbox profile JSON: {0}")]
        ProfileJson(#[from] serde_json::Error),
        #[error("unsupported sandbox profile: {0}")]
        UnsupportedProfile(&'static str),
        #[error("{0}")]
        LandlockUnavailable(&'static str),
        #[error("Landlock setup failed: {0}")]
        Landlock(#[from] landlock::RulesetError),
        #[error("bubblewrap is missing at {BWRAP_PROGRAM}")]
        BwrapMissing,
        #[error("failed to resolve current helper executable for bwrap fallback: {0}")]
        CurrentExe(std::io::Error),
        #[error("bwrap fallback paths must be absolute, got {0:?}")]
        RelativeBwrapPath(PathBuf),
        #[error(
            "bwrap fallback cannot inject caller env without exposing secrets via argv; require Linux 6.7+ Landlock for this profile"
        )]
        BwrapCallerEnvUnsupported,
        #[error("bwrap fallback failed to exec {BWRAP_PROGRAM}: {source}")]
        BwrapExec { source: std::io::Error },
        #[error("seccomp filter setup failed: {0}")]
        SeccompBackend(#[from] seccompiler::BackendError),
        #[error("seccomp filter installation failed: {0}")]
        Seccomp(#[from] seccompiler::Error),
        #[error("unsupported seccomp target architecture: {0}")]
        UnsupportedArchitecture(&'static str),
        #[error("sandbox command must be an absolute path, got {0:?}")]
        RelativeCommand(OsString),
        #[error("failed to scan inherited file descriptors: {0}")]
        FdScan(std::io::Error),
        #[error("failed to read RLIMIT_NOFILE while closing inherited file descriptors: {0}")]
        FdLimit(Errno),
        #[error("failed to close inherited file descriptor {fd}: {error}")]
        FdClose { fd: RawFd, error: Errno },
        #[error("failed to exec command {command:?}: {source}")]
        Exec {
            command: OsString,
            source: std::io::Error,
        },
    }

    #[cfg(test)]
    #[path = "tests.rs"]
    mod tests;
}
