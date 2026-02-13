//! Sub-module where all attaching APIs are located.
//!
//! [`unix_socket`] exposes the attachment functions for communication with a UNIX socket.

#[cfg(unix)]
pub mod unix_socket;
#[cfg(windows)]
pub mod windows_unix_socket;

pub mod attacher;

// Decide which communication channel is the default
#[cfg(unix)]
pub use unix_socket::{connect, listen};
#[cfg(windows)]
pub use windows_unix_socket::{connect, listen};
