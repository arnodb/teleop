//! Sub-module where all attaching APIs are located.
//!
//! [`unix_socket`] exposes the attachment functions for communication with a UNIX socket.

use std::future::Future;

#[cfg(any(unix, doc))]
pub mod unix_socket;

#[cfg(any(unix, doc))]
pub mod unix_attacher;

pub trait Attacher {
    type SignalGuard;

    fn send_signal(
        pid: u32,
    ) -> impl Future<Output = Result<Self::SignalGuard, Box<dyn std::error::Error>>>;

    fn await_signal() -> impl Future<Output = Result<(), Box<dyn std::error::Error>>>;
}
