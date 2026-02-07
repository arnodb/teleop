//! Sub-module where all attaching APIs are located.
//!
//! [`unix_socket`] exposes the attachment functions for communication with a UNIX socket.

use std::future::Future;

#[cfg(any(unix, doc))]
pub mod unix_socket;

pub mod dummy_attacher;
#[cfg(feature = "inotify")]
pub mod inotify_attacher;
#[cfg(any(unix, doc))]
pub mod unix_attacher;

pub trait Attacher {
    type Signal: AttacherSignal;

    fn signal(pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>>;

    fn signaled() -> impl Future<Output = Result<(), Box<dyn std::error::Error>>>;
}

pub trait AttacherSignal {
    fn send(&mut self) -> impl Future<Output = Result<(), Box<dyn std::error::Error>>>;
}

// Decide which attacher is the default

#[cfg(windows)]
pub use dummy_attacher::DummyAttacher as DefaultAttacher;
#[cfg(feature = "inotify")]
pub use inotify_attacher::InotifyAttacher as DefaultAttacher;
#[cfg(all(unix, not(feature = "inotify")))]
pub use unix_attacher::UnixAttacher as DefaultAttacher;
