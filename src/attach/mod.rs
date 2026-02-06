//! Sub-module where all attaching APIs are located.
//!
//! [`unix_socket`] exposes the attachment functions for communication with a UNIX socket.

use std::future::Future;

#[cfg(any(unix, doc))]
pub mod unix_socket;

#[cfg(any(unix, doc))]
pub mod unix_attacher;

pub trait Attacher {
    type Signal: AttacherSignal;

    fn signal(pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>>;

    fn signaled() -> impl Future<Output = Result<(), Box<dyn std::error::Error>>>;
}

pub trait AttacherSignal {
    fn send(&self) -> impl Future<Output = Result<(), Box<dyn std::error::Error>>>;
}

// Decide which attacher is the default

#[cfg(any(unix, doc))]
pub use unix_attacher::UnixAttacher as DefaultAttacher;
