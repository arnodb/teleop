//! Teleop provides a means to attach to a local process knowing its ID, and then provides RPC
//! capabilities to the client.
//!
//! It currently only supports UNIX socket and Cap’n Proto RPC, but it is aimed at providing more
//! ways to attach to a process and to communicate with it.
//!
//! ## UNIX socket
//!
//! The implementation is very much inspired by Java [Attach
//! API](https://docs.oracle.com/javase/8/docs/technotes/guides/attach/index.html):
//!
//! * the process to be teleoperated waits for a signal
//! * if some conditions are met then it opens the UNIX socket at a known location
//! * the client can then connect to the unix socket and use the RPC protocol set up by the remote
//!   process
//!
//! ## Cap'n Proto RPC
//!
//! Teleop provides a root interface named `Teleop` (see `teleop.capnp`) which gives access to
//! arbitrary services.
//!
//! ## Example
//!
//! See examples in the Git repository.
//!
//! * The server example shows how to setup the process to teleoperate, including an `echo` service
//!   which will reply to a request by echoing the input.
//! * The client example shows how to setup the client, initiate the attach process, request the
//!   `echo` service, and send echo requests.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod attach;
pub mod operate;

mod internal;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    // The attacher tests need to run separately
    pub(crate) static ATTACH_PROCESS_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
