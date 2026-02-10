//! Sub-module where all attaching APIs are located.
//!
//! [`unix_socket`] exposes the attachment functions for communication with a UNIX socket.

use std::future::Future;

#[cfg(unix)]
pub mod unix_socket;
#[cfg(windows)]
pub mod windows_unix_socket;

pub mod dummy_attacher;
#[cfg(feature = "inotify")]
pub mod inotify_attacher;
#[cfg(target_os = "macos")]
pub mod kqueue_attacher;
#[cfg(unix)]
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
#[cfg(target_os = "macos")]
pub use kqueue_attacher::KqueueAttacher as DefaultAttacher;
#[cfg(all(unix, not(target_os = "macos"), not(feature = "inotify")))]
pub use unix_attacher::UnixAttacher as DefaultAttacher;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{pin::pin, time::Duration};

    use async_io::Timer;
    use futures::{select, FutureExt};
    use futures_lite::future::or;

    use super::Attacher;
    use crate::attach::AttacherSignal;

    // The attacher tests need to run separately
    static ATTACHER_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) fn test_attacher<A>()
    where
        A: Attacher,
    {
        let _attacher_test = ATTACHER_TEST_MUTEX.lock();

        let mut exec = futures::executor::LocalPool::new();

        let res = exec.run_until(or(
            async {
                let signaled = A::signaled();
                let mut signal = A::signal(std::process::id())?;
                signal.send().await?;
                signaled.await?;
                drop(signal);

                let mut signaled = pin!(A::signaled().fuse());
                select! {
                    res = signaled => {
                        res?;
                        panic!("Should not be signaled yet");
                    }
                    _ = Timer::after(Duration::from_millis(500)).fuse() => ()
                };

                let mut signal = A::signal(std::process::id())?;
                signal.send().await?;
                signaled.await?;
                drop(signal);

                Ok::<_, Box<dyn std::error::Error>>(())
            },
            Timer::after(Duration::from_secs(5)).then(async |_| Err("Test timeout".into())),
        ));

        exec.run();

        res.unwrap();
    }
}
