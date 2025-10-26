//! Sub-module where all attaching APIs are located.
//!
//! [`unix_socket`] exposes the attachment functions for communication with a UNIX socket.

#[cfg(any(unix, doc))]
pub mod unix_socket;
