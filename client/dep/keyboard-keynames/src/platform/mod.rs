//! The KeyLayout struct and supporting elements
#[cfg(target_os = "linux")]
#[path = "unix/mod.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "windows/mod.rs"]
mod platform;

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
#[path = "other/mod.rs"]
mod platform;

pub use self::platform::key_layout::*;
