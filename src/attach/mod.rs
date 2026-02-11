//! Sub-module where all attaching APIs are located.
//!
//! See available sub-modules for your platform.
//!
//! The default communication channel may vary from one platform to another ([`listen`], [`connect`]).

#[cfg(unix)]
pub mod unix_socket;
#[cfg(windows)]
pub mod windows_named_pipe;
#[cfg(windows)]
pub mod windows_unix_socket;

pub mod attacher;

// Decide which communication channel is the default
#[cfg(unix)]
pub use unix_socket::{connect, listen};
#[cfg(windows)]
pub use windows_named_pipe::{connect, listen};
