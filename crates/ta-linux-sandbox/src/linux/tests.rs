use super::*;
use nix::{
    fcntl::{FcntlArg, FdFlag, fcntl},
    sys::socket::{AddressFamily, SockFlag, SockType, SockaddrIn, connect, socket},
};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    os::fd::AsRawFd,
    process::Command,
    str::FromStr,
};

#[path = "bwrap_env_tests.rs"]
mod bwrap_env_tests;

#[test]
fn parses_profile_and_command_args() {
    let profile = SandboxProfile::new()
        .read_path("/tmp/read")
        .write_path("/tmp/write")
        .network(NetworkPolicy::Off);
    let profile_json = serde_json::to_string(&profile).expect("profile json");

    let invocation = Invocation::parse([
        OsString::from("ta-linux-sandbox"),
        OsString::from(PROFILE_ARG),
        OsString::from(profile_json),
        OsString::from(ARG_SEPARATOR),
        OsString::from("/bin/true"),
        OsString::from("--flag"),
    ])
    .expect("parse invocation");

    assert_eq!(invocation.profile, profile);
    assert_eq!(invocation.command, OsString::from("/bin/true"));
    assert_eq!(invocation.args, [OsString::from("--flag")]);
}

#[test]
fn rejects_relative_command_paths_before_sandbox_setup() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Open);
    let profile_json = serde_json::to_string(&profile).expect("profile json");
    let error = run([
        OsString::from("ta-linux-sandbox"),
        OsString::from(PROFILE_ARG),
        OsString::from(profile_json),
        OsString::from(ARG_SEPARATOR),
        OsString::from("true"),
    ])
    .expect_err("relative command");

    assert!(matches!(error, HelperError::RelativeCommand(_)));
}

#[test]
fn rejects_loopback_before_landlock_setup() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Loopback);
    let error = landlock_network(&profile).expect_err("unsupported network profile");

    assert!(matches!(
        error,
        HelperError::UnsupportedProfile(message)
            if message.contains("Loopback") && message.contains("address-aware Linux enforcement")
    ));
}

#[test]
fn rejects_hostname_allowlist_before_landlock_setup() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Allowlist(vec!["localhost".into()]));
    let error = landlock_network(&profile).expect_err("unsupported hostname allowlist");

    assert!(matches!(
        error,
        HelperError::UnsupportedProfile(message)
            if message.contains("TCP port") && message.contains("hostname")
    ));
}

#[test]
fn decide_sandbox_strategy_uses_bwrap_when_landlock_unavailable() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Off);

    let strategy =
        decide_sandbox_strategy(&profile, |_| Ok(false)).expect("decide bwrap fallback strategy");

    assert_eq!(strategy, SandboxStrategy::Bwrap);
}

#[test]
fn decide_sandbox_strategy_uses_landlock_when_capability_available() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Off);

    let strategy =
        decide_sandbox_strategy(&profile, |_| Ok(true)).expect("decide landlock strategy");

    assert_eq!(
        strategy,
        SandboxStrategy::Landlock(LandlockNetwork::DenyTcp)
    );
}

#[test]
fn decide_sandbox_strategy_rejects_bwrap_for_unsupported_network() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Open);
    let error = decide_sandbox_strategy(&profile, |_| Ok(false))
        .expect_err("network: Open must not fall back to bwrap");

    assert!(matches!(
        error,
        HelperError::UnsupportedProfile(message)
            if message.contains("bwrap fallback supports only network: Off")
    ));
}

#[test]
fn decide_sandbox_strategy_rejects_bwrap_for_tcp_allowlist_when_landlock_unavailable() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Allowlist(vec!["443".into()]));
    let error = decide_sandbox_strategy(&profile, |_| Ok(false))
        .expect_err("TCP allowlist must not fall back to bwrap");

    assert!(matches!(
        error,
        HelperError::UnsupportedProfile(message)
            if message.contains("bwrap fallback supports only network: Off")
    ));
}

#[test]
fn run_falls_back_to_bwrap_before_landlock_mutation_when_capability_probe_fails() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Off);
    let profile_json = serde_json::to_string(&profile).expect("profile json");
    let events = std::cell::RefCell::new(Vec::new());

    run_with_hooks(
        [
            OsString::from("ta-linux-sandbox"),
            OsString::from(PROFILE_ARG),
            OsString::from(profile_json),
            OsString::from(ARG_SEPARATOR),
            OsString::from("/bin/true"),
        ],
        RunHooks {
            close_inherited_fds: || {
                events.borrow_mut().push("fd_hygiene");
                Ok(())
            },
            capability_probe: |network: &LandlockNetwork| {
                assert_eq!(network, &LandlockNetwork::DenyTcp);
                events.borrow_mut().push("decide_strategy");
                Ok(false)
            },
            apply_profile: |_: &SandboxProfile, _: &LandlockNetwork| {
                events.borrow_mut().push("apply_profile");
                panic!("bwrap fallback must not apply Landlock");
            },
            exec_bwrap_fallback: |_: &Invocation| {
                events.borrow_mut().push("bwrap");
                Ok(())
            },
            exec_command: |_, _| {
                events.borrow_mut().push("exec_command");
                panic!("bwrap fallback must return before direct exec");
            },
        },
    )
    .expect("run should use bwrap fallback");

    assert_eq!(
        events.into_inner(),
        ["fd_hygiene", "decide_strategy", "bwrap"]
    );
}

#[test]
fn parses_tcp_port_allowlist_entries() {
    assert_eq!(
        parse_tcp_allowlist(&["443".into(), "tcp:443".into(), "22".into()]).expect("ports"),
        [22, 443]
    );
}

#[test]
fn bwrap_fallback_rejects_network_open() {
    let invocation = test_invocation(
        SandboxProfile::new().network(NetworkPolicy::Open),
        "/bin/true",
        &[],
    );
    let error = exec_bwrap_fallback(&invocation).expect_err("network: Open must fail closed");

    assert!(matches!(
        error,
        HelperError::UnsupportedProfile(message)
            if message.contains("bwrap fallback supports only network: Off")
    ));
}

#[test]
fn fd_scan_unavailable_errors_use_rlimit_fallback() {
    for kind in [
        std::io::ErrorKind::NotFound,
        std::io::ErrorKind::PermissionDenied,
    ] {
        let error = std::io::Error::from(kind);

        assert!(should_fallback_to_rlimit(&error));
    }
}

#[test]
fn read_dir_non_fallback_errors_are_fd_scan() {
    type EmptyFdEntries = std::iter::Empty<Result<fs::DirEntry, std::io::Error>>;
    let error = close_inherited_fds_with(
        Err::<EmptyFdEntries, _>(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
        || panic!("unexpected RLIMIT fallback"),
    )
    .expect_err("read_dir error should fail closed");

    assert!(matches!(
        error,
        HelperError::FdScan(error) if error.kind() == std::io::ErrorKind::BrokenPipe
    ));
}

#[test]
fn read_dir_not_found_uses_rlimit_fallback() {
    type EmptyFdEntries = std::iter::Empty<Result<fs::DirEntry, std::io::Error>>;
    let error = close_inherited_fds_with(
        Err::<EmptyFdEntries, _>(std::io::Error::from(std::io::ErrorKind::NotFound)),
        || Err(HelperError::FdLimit(Errno::EINVAL)),
    )
    .expect_err("fallback marker");

    assert!(matches!(error, HelperError::FdLimit(Errno::EINVAL)));
}

#[test]
fn fd_scan_iterator_errors_are_fd_scan() {
    let entries = std::iter::once(Err::<fs::DirEntry, _>(std::io::Error::from(
        std::io::ErrorKind::PermissionDenied,
    )));
    let error = close_inherited_fds_with(Ok(entries), || panic!("unexpected fallback"))
        .expect_err("iterator error should fail closed");

    assert!(matches!(
        error,
        HelperError::FdScan(error)
            if error.kind() == std::io::ErrorKind::PermissionDenied
    ));
}

#[test]
fn closes_inherited_non_cloexec_fd_before_sandbox_and_exec() {
    let mut file = tempfile::tempfile().expect("temporary fd");
    let mut second_file = tempfile::tempfile().expect("second temporary fd");
    file.write_all(b"secret").expect("write fd content");
    second_file
        .write_all(b"secret")
        .expect("write second fd content");
    file.seek(SeekFrom::Start(0)).expect("rewind fd");
    second_file
        .seek(SeekFrom::Start(0))
        .expect("rewind second fd");
    fcntl(&file, FcntlArg::F_SETFD(FdFlag::empty())).expect("clear close-on-exec");
    fcntl(&second_file, FcntlArg::F_SETFD(FdFlag::empty()))
        .expect("clear close-on-exec on second fd");
    let inherited_fds = format!("{}:{}", file.as_raw_fd(), second_file.as_raw_fd());

    let output = run_ignored_helper(
        "linux::tests::helper_closes_inherited_fd_before_exec",
        vec![("TA_SANDBOX_TEST_FDS", OsString::from(inherited_fds))],
    );
    if handle_sandbox_unavailable(&output, "FD hygiene execution test") {
        return;
    }

    assert!(
        output.status.success(),
        "sandbox helper leaked an inherited fd\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn network_open_allows_ipv4_tcp_socket_and_off_denies_it() {
    let output = run_ignored_helper(
        "linux::tests::landlock_helper_allows_ipv4_tcp_socket_with_network_open",
        Vec::<(&str, OsString)>::new(),
    );
    if handle_sandbox_unavailable(&output, "network: Open socket probe") {
        return;
    }
    assert!(
        output.status.success(),
        "network: Open did not permit AF_INET/SOCK_STREAM socket creation\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_ignored_helper(
        "linux::tests::landlock_helper_denies_ipv4_tcp_socket_with_network_off",
        Vec::<(&str, OsString)>::new(),
    );
    if handle_sandbox_unavailable(&output, "network: Off socket probe") {
        return;
    }
    assert!(
        output.status.success(),
        "network: Off did not deny AF_INET/SOCK_STREAM socket creation with EPERM\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn network_allowlist_allows_configured_tcp_port_and_denies_other_ports() {
    let output = run_ignored_helper(
        "linux::tests::landlock_helper_allows_tcp_connect_to_allowlisted_port",
        Vec::<(&str, OsString)>::new(),
    );
    if handle_sandbox_unavailable(&output, "network: Allowlist port 443 connect probe") {
        return;
    }
    assert!(
        output.status.success(),
        "network: Allowlist did not permit connect attempts to TCP port 443\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_ignored_helper(
        "linux::tests::landlock_helper_denies_tcp_connect_to_unlisted_port",
        Vec::<(&str, OsString)>::new(),
    );
    if handle_sandbox_unavailable(&output, "network: Allowlist port 22 denial probe") {
        return;
    }
    assert!(
        output.status.success(),
        "network: Allowlist did not deny connect attempts to TCP port 22\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn landlock_allows_true_with_tmp_read_write_profile() {
    let output = run_ignored_helper(
        "linux::tests::landlock_helper_allows_true",
        Vec::<(&str, OsString)>::new(),
    );
    if handle_sandbox_unavailable(&output, "Landlock execution test") {
        return;
    }

    assert!(
        output.status.success(),
        "sandboxed /bin/true failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bwrap_fallback_allows_true_with_network_off() {
    let output = run_ignored_helper(
        "linux::tests::bwrap_helper_allows_true",
        Vec::<(&str, OsString)>::new(),
    );
    if handle_bwrap_unavailable(&output, "bwrap fallback execution test") {
        return;
    }

    assert!(
        output.status.success(),
        "bwrap fallback did not run /bin/true\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bwrap_exec_clears_helper_parent_env() {
    let output = run_ignored_helper(
        "linux::tests::helper_execs_mock_bwrap_with_clean_env",
        vec![
            (
                "TA_MOCK_BWRAP",
                std::env::current_exe()
                    .expect("test binary path")
                    .into_os_string(),
            ),
            ("TA_BWRAP_LEAK_TEST", OsString::from("secret")),
        ],
    );

    assert!(
        output.status.success(),
        "mock bwrap env probe failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn landlock_denies_fake_home_ssh_read() {
    let home = tempfile::tempdir().expect("fake home");
    let ssh_dir = home.path().join(".ssh");
    fs::create_dir(&ssh_dir).expect("create fake ssh dir");
    fs::write(ssh_dir.join("secret"), "secret").expect("write fake ssh secret");

    let output = run_ignored_helper(
        "linux::tests::landlock_helper_denies_fake_home_ssh_read",
        vec![("HOME", home.path().as_os_str().to_owned())],
    );
    if handle_sandbox_unavailable(&output, "Landlock denial test") {
        return;
    }

    assert!(
        output.status.success(),
        "sandboxed ~/.ssh read denial probe failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_ignored_helper(helper_name: &str, envs: Vec<(&str, OsString)>) -> std::process::Output {
    let mut command = Command::new(std::env::current_exe().expect("test binary path"));
    command
        .arg("--ignored")
        .arg("--exact")
        .arg(helper_name)
        .arg("--nocapture");
    for (name, value) in envs {
        command.env(name, value);
    }
    command.output().expect("run ignored helper")
}

fn landlock_unavailable(output: &std::process::Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("Landlock ruleset was not enforced")
        || stderr.contains("Landlock setup failed")
        || stderr.contains("seccomp filter installation failed")
        || stderr.contains("bwrap fallback supports only network: Off")
        || bwrap_unavailable(output)
}

fn handle_sandbox_unavailable(output: &std::process::Output, test_name: &str) -> bool {
    if !landlock_unavailable(output) {
        return false;
    }

    let message = format!(
        "skipping {test_name}: Linux sandbox support unavailable; set TA_REQUIRE_LINUX_SANDBOX=1 in CI to fail when Landlock/seccomp enforcement is missing"
    );
    if matches!(env::var("TA_REQUIRE_LINUX_SANDBOX").as_deref(), Ok("1")) {
        panic!(
            "{message}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    eprintln!("{message}");
    true
}

fn bwrap_unavailable(output: &std::process::Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("bubblewrap is missing")
        || stderr.contains("No permissions to create a new namespace")
        || stderr.contains("setting up uid map")
        || stderr.contains("Operation not permitted")
        || stderr.contains("Creating new namespace failed")
}

fn handle_bwrap_unavailable(output: &std::process::Output, test_name: &str) -> bool {
    if !bwrap_unavailable(output) {
        return false;
    }

    let message = format!(
        "skipping {test_name}: bwrap fallback support unavailable; set TA_REQUIRE_LINUX_SANDBOX=1 in CI to fail when bubblewrap/user namespaces are missing"
    );
    if matches!(env::var("TA_REQUIRE_LINUX_SANDBOX").as_deref(), Ok("1")) {
        panic!(
            "{message}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    eprintln!("{message}");
    true
}

#[test]
#[ignore = "spawned as a subprocess by landlock_allows_true_with_tmp_read_write_profile"]
fn landlock_helper_allows_true() {
    let allowed = tempfile::tempdir().expect("allowed tmpdir");
    let profile = SandboxProfile::new()
        .read_path(allowed.path())
        .write_path(allowed.path())
        .network(NetworkPolicy::Off);
    run_helper_or_panic(profile, "/bin/true", []);
}

#[test]
#[ignore = "spawned as a subprocess by bwrap_fallback_allows_true_with_network_off"]
fn bwrap_helper_allows_true() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Off);
    let invocation = test_invocation(profile, "/bin/true", &[]);
    if let Err(error) = exec_bwrap_fallback(&invocation) {
        panic!("{error}");
    }
}

#[test]
#[ignore = "spawned as a subprocess by network_open_allows_ipv4_tcp_socket_and_off_denies_it"]
fn landlock_helper_allows_ipv4_tcp_socket_with_network_open() {
    run_current_test_binary_under_sandbox(
        NetworkPolicy::Open,
        "linux::tests::socket_probe_succeeds",
    );
}

#[test]
#[ignore = "spawned as a subprocess by network_open_allows_ipv4_tcp_socket_and_off_denies_it"]
fn landlock_helper_denies_ipv4_tcp_socket_with_network_off() {
    run_current_test_binary_under_sandbox(
        NetworkPolicy::Off,
        "linux::tests::socket_probe_gets_eperm",
    );
}

#[test]
#[ignore = "spawned as a subprocess by network_allowlist_allows_configured_tcp_port_and_denies_other_ports"]
fn landlock_helper_allows_tcp_connect_to_allowlisted_port() {
    run_current_test_binary_under_sandbox(
        NetworkPolicy::Allowlist(vec!["443".into()]),
        "linux::tests::connect_probe_port_443_is_not_sandbox_denied",
    );
}

#[test]
#[ignore = "spawned as a subprocess by network_allowlist_allows_configured_tcp_port_and_denies_other_ports"]
fn landlock_helper_denies_tcp_connect_to_unlisted_port() {
    run_current_test_binary_under_sandbox(
        NetworkPolicy::Allowlist(vec!["443".into()]),
        "linux::tests::connect_probe_port_22_is_sandbox_denied",
    );
}

#[test]
#[ignore = "spawned as a sandboxed subprocess by landlock_helper_allows_ipv4_tcp_socket_with_network_open"]
fn socket_probe_succeeds() {
    socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .expect("AF_INET/SOCK_STREAM socket should be allowed");
}

#[test]
#[ignore = "spawned as a sandboxed subprocess by landlock_helper_denies_ipv4_tcp_socket_with_network_off"]
fn socket_probe_gets_eperm() {
    let error = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .expect_err("AF_INET/SOCK_STREAM socket should be denied");

    assert_eq!(error, Errno::EPERM);
}

#[test]
#[ignore = "spawned as a sandboxed subprocess by landlock_helper_allows_tcp_connect_to_allowlisted_port"]
fn connect_probe_port_443_is_not_sandbox_denied() {
    let result = connect_to_localhost_port(443);
    assert!(
        !matches!(result, Err(Errno::EACCES | Errno::EPERM)),
        "connect to allowlisted TCP port 443 should not be sandbox-denied: {result:?}"
    );
}

#[test]
#[ignore = "spawned as a sandboxed subprocess by landlock_helper_denies_tcp_connect_to_unlisted_port"]
fn connect_probe_port_22_is_sandbox_denied() {
    let error = connect_to_localhost_port(22).expect_err("TCP port 22 should be sandbox-denied");
    assert!(
        matches!(error, Errno::EACCES | Errno::EPERM),
        "connect to unlisted TCP port 22 should be sandbox-denied, got {error}"
    );
}

fn connect_to_localhost_port(port: u16) -> nix::Result<()> {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::SOCK_NONBLOCK,
        None,
    )?;
    let addr = SockaddrIn::from_str(&format!("127.0.0.1:{port}")).expect("localhost addr");
    connect(fd.as_raw_fd(), &addr)
}

#[test]
#[ignore = "spawned as a subprocess by landlock_denies_fake_home_ssh_read"]
fn landlock_helper_denies_fake_home_ssh_read() {
    let allowed = tempfile::tempdir().expect("allowed tmpdir");
    let profile = SandboxProfile::new()
        .read_path(allowed.path())
        .write_path(allowed.path())
        .network(NetworkPolicy::Off);
    run_helper_or_panic(
        profile,
        "/bin/sh",
        [
            "-c",
            r#"cat "$HOME/.ssh/secret" >/dev/null 2>&1; test $? -ne 0"#,
        ],
    );
}

#[test]
#[ignore = "spawned as a subprocess by closes_inherited_non_cloexec_fd_before_sandbox_and_exec"]
fn helper_closes_inherited_fd_before_exec() {
    let fds = env::var("TA_SANDBOX_TEST_FDS").expect("test fds env");
    for fd in fds.split(':') {
        assert!(
            Path::new(&format!("/proc/self/fd/{fd}")).exists(),
            "test fd {fd} was not inherited before helper fd hygiene ran"
        );
    }
    let script = fds
        .split(':')
        .map(|fd| format!("cat <&{fd} >/dev/null 2>&1; test $? -ne 0"))
        .collect::<Vec<_>>()
        .join(" && ");
    let allowed = tempfile::tempdir().expect("allowed tmpdir");
    let profile = SandboxProfile::new()
        .read_path(allowed.path())
        .write_path(allowed.path())
        .network(NetworkPolicy::Off);
    run_helper_or_panic(profile, "/bin/sh", ["-c", script.as_str()]);
}

#[test]
#[ignore = "spawned as a subprocess by bwrap_exec_clears_helper_parent_env"]
fn helper_execs_mock_bwrap_with_clean_env() {
    let mock_bwrap = env::var_os("TA_MOCK_BWRAP").expect("mock bwrap path env");
    let source = exec_bwrap_process(
        Path::new(&mock_bwrap),
        vec![
            OsString::from("--ignored"),
            OsString::from("--exact"),
            OsString::from("linux::tests::mock_bwrap_env_probe"),
            OsString::from("--nocapture"),
        ],
    );
    panic!("mock bwrap exec failed: {source}");
}

#[test]
#[ignore = "execed as mock bwrap by helper_execs_mock_bwrap_with_clean_env"]
fn mock_bwrap_env_probe() {
    assert_eq!(
        env::var_os("PATH"),
        Some(OsString::from(BWRAP_PROCESS_PATH))
    );
    let leaked_env: Vec<_> = env::vars_os().filter(|(name, _)| name != "PATH").collect();
    let leaked_names: Vec<_> = leaked_env.iter().map(|(name, _)| name).collect();
    assert!(
        leaked_env.is_empty(),
        "mock bwrap process received non-base env: count={}, names={:?}",
        leaked_env.len(),
        leaked_names
    );
}

fn run_current_test_binary_under_sandbox(network: NetworkPolicy, probe_name: &str) {
    let test_binary = std::env::current_exe().expect("test binary path");
    let test_binary_dir = test_binary.parent().expect("test binary dir");
    let profile = SandboxProfile::new()
        .read_path(test_binary_dir)
        .network(network);
    let command = test_binary.to_str().expect("test binary path is UTF-8");
    run_helper_or_panic(
        profile,
        command,
        ["--ignored", "--exact", probe_name, "--nocapture"],
    );
}

fn run_helper_or_panic<const N: usize>(
    profile: SandboxProfile,
    command: &str,
    command_args: [&str; N],
) {
    let mut args = vec![
        OsString::from("ta-linux-sandbox"),
        OsString::from(PROFILE_ARG),
        OsString::from(serde_json::to_string(&profile).expect("profile json")),
        OsString::from(ARG_SEPARATOR),
        OsString::from(command),
    ];
    args.extend(command_args.into_iter().map(OsString::from));

    if let Err(error) = run(args) {
        panic!("{error}");
    }
}

fn bwrap_args_contain_env(args: &[OsString], name: &str, value: &str) -> bool {
    args.windows(3)
        .any(|window| window[0] == "--setenv" && window[1] == name && window[2] == value)
}

fn test_invocation(profile: SandboxProfile, command: &str, command_args: &[&str]) -> Invocation {
    let profile_json = OsString::from(serde_json::to_string(&profile).expect("profile json"));
    Invocation {
        profile_json,
        profile,
        command: OsString::from(command),
        args: command_args.iter().copied().map(OsString::from).collect(),
        bwrap_child: false,
        caller_env_present: false,
    }
}
