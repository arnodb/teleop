//! Attachment mechanisms.
//!
//! The default attacher may vary from one platform to another.

pub mod dummy;
#[cfg(feature = "inotify")]
pub mod inotify;
#[cfg(target_os = "macos")]
pub mod kqueue;
#[cfg(unix)]
pub mod unix;

use std::future::Future;

// Decide which attacher is the default
#[cfg(windows)]
pub use dummy::DummyAttacher as DefaultAttacher;
#[cfg(feature = "inotify")]
pub use inotify::InotifyAttacher as DefaultAttacher;
#[cfg(target_os = "macos")]
pub use kqueue::KqueueAttacher as DefaultAttacher;
#[cfg(all(unix, not(target_os = "macos"), not(feature = "inotify")))]
pub use unix::UnixAttacher as DefaultAttacher;

/// Attacher abstraction.
pub trait Attacher {
    /// The type of signal returned by [signal](`Attacher::signal`).
    type Signal: AttacherSignal;

    /// Returns a signal which can be sent multiple times to the target process.
    fn signal(pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>>;

    /// Waits asynchronously for the signal to be received by the process.
    fn signaled() -> impl Future<Output = Result<(), Box<dyn std::error::Error>>>;
}

/// Attachment signal abstraction.
pub trait AttacherSignal {
    /// Sends the signal asynchronously once.
    fn send(&mut self) -> impl Future<Output = Result<(), Box<dyn std::error::Error>>>;
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        time::{Duration, Instant},
    };

    use async_io::Timer;
    use futures::{select, FutureExt};

    use super::{Attacher, AttacherSignal};
    use crate::tests::ATTACH_PROCESS_TEST_MUTEX;

    #[cfg_attr(windows, allow(unused))]
    pub(crate) fn test_attacher<A, W>(wrong_signal: W)
    where
        A: Attacher,
        W: Future<Output = ()>,
    {
        let _attacher_test = ATTACH_PROCESS_TEST_MUTEX.lock();

        let mut exec = futures::executor::LocalPool::new();

        let res = exec.run_until(async {
            let job = async {
                let signaled = A::signaled();
                let mut signal = A::signal(std::process::id())?;
                signal.send().await?;
                signaled.await?;
                drop(signal);

                let mut signaled = pin!(A::signaled().fuse());
                let mut full_timer = Timer::at(Instant::now() + Duration::from_millis(500)).fuse();
                select! {
                    // Wait so that signaled is polled
                    () = Timer::after(Duration::from_millis(10))
                        .then(async |_| wrong_signal.await).fuse() => {}
                    res = signaled => {
                        res?;
                        panic!("Should not be signaled yet (wrong signal)");
                    }
                };
                select! {
                    res = signaled => {
                        res?;
                        panic!("Should not be signaled yet");
                    }
                    _ = full_timer => {}
                };

                let mut signal = A::signal(std::process::id())?;
                signal.send().await?;
                signaled.await?;
                drop(signal);

                Ok::<_, Box<dyn std::error::Error>>(())
            };

            let timeout =
                Timer::after(Duration::from_secs(5)).then(async |_| Err("Test timeout".into()));

            select! {
                a = job.fuse() => a,
                b = timeout.fuse() => b,
            }
        });

        exec.run();

        res.unwrap();
    }
}
