use super::*;
use std::{ffi::OsString, path::PathBuf};

#[test]
fn bwrap_fallback_rejects_caller_env() {
    let profile = SandboxProfile::new().network(NetworkPolicy::Off);
    let profile_json = serde_json::to_string(&profile).expect("profile json");
    let error = run_with_hooks(
        [
            OsString::from("ta-linux-sandbox"),
            OsString::from(LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG),
            OsString::from(PROFILE_ARG),
            OsString::from(profile_json),
            OsString::from(ARG_SEPARATOR),
            OsString::from("/bin/true"),
        ],
        RunHooks {
            close_inherited_fds: || Ok(()),
            capability_probe: |_: &LandlockNetwork| Ok(false),
            apply_profile: |_: &SandboxProfile, _: &LandlockNetwork| {
                panic!("Landlock must be unavailable in this test")
            },
            exec_bwrap_fallback: |invocation: &Invocation| {
                build_bwrap_args_with_env(
                    invocation,
                    PathBuf::from("/opt/taugentic/ta-linux-sandbox"),
                    |_| None,
                )
                .map(|_| ())
            },
            exec_command: |_, _| panic!("bwrap fallback should return before exec"),
        },
    )
    .expect_err("caller env must reject bwrap fallback");

    assert!(matches!(error, HelperError::BwrapCallerEnvUnsupported));
}

#[test]
fn bwrap_fallback_does_not_setenv_caller_secret_in_argv() {
    let invocation = test_invocation(
        SandboxProfile::new()
            .env("PATH")
            .network(NetworkPolicy::Off),
        "/bin/true",
        &[],
    );
    let args = build_bwrap_args_with_env(
        &invocation,
        PathBuf::from("/opt/taugentic/ta-linux-sandbox"),
        base_env,
    )
    .expect("bwrap args");

    assert!(bwrap_args_contain_env(&args, "PATH", "/usr/bin"));
    assert!(!args.iter().any(|arg| arg == "TA_CALLER_TOKEN"));
    assert!(!args.iter().any(|arg| arg == "caller-secret"));
}

#[test]
fn bwrap_fallback_setenvs_only_base_allowlist_vars() {
    let invocation = test_invocation(
        SandboxProfile::new()
            .env("PATH")
            .env("HOME")
            .env("LANG")
            .env("TA_CALLER_TOKEN")
            .network(NetworkPolicy::Off),
        "/bin/true",
        &[],
    );
    let args = build_bwrap_args_with_env(
        &invocation,
        PathBuf::from("/opt/taugentic/ta-linux-sandbox"),
        base_env,
    )
    .expect("bwrap args");

    assert_eq!(
        bwrap_env_pairs(&args),
        [
            (OsString::from("PATH"), OsString::from("/usr/bin")),
            (OsString::from("HOME"), OsString::from("/home/taugentic")),
            (OsString::from("LANG"), OsString::from("C.UTF-8")),
        ]
    );
}

fn base_env(name: &str) -> Option<OsString> {
    [
        ("PATH", "/usr/bin"),
        ("HOME", "/home/taugentic"),
        ("LANG", "C.UTF-8"),
        ("TA_CALLER_TOKEN", "caller-secret"),
    ]
    .into_iter()
    .find_map(|(key, value)| (key == name).then(|| OsString::from(value)))
}

fn bwrap_env_pairs(args: &[OsString]) -> Vec<(OsString, OsString)> {
    args.windows(3)
        .filter(|window| window[0] == "--setenv")
        .map(|window| (window[1].clone(), window[2].clone()))
        .collect()
}
