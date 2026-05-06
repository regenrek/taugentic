#[cfg(any(
    test,
    target_os = "linux",
    not(any(target_os = "macos", target_os = "linux", target_os = "windows"))
))]
pub(crate) mod memory;

#[cfg(target_os = "linux")]
pub(crate) mod linux;

#[cfg(target_os = "macos")]
pub(crate) mod macos;

#[cfg(target_os = "windows")]
pub(crate) mod windows;
