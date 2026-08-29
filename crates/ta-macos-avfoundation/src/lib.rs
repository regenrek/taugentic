//! The sole private, safe boundary around macOS AVFoundation audio mechanics.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod macos;

pub use macos::*;
