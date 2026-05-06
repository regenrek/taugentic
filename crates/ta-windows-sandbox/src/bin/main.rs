//! Windows sandbox helper entrypoint.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    windows::main()
}

#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("ta-windows-sandbox: Windows sandbox helper only runs on Windows");
    std::process::ExitCode::from(126)
}
