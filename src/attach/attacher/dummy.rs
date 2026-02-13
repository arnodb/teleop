//! Dummy attacher which listens immediately.

use crate::attach::attacher::{Attacher, AttacherSignal};

/// Dummy attacher.
///
/// It does nothing and considers the signal as signaled from the very beginning.
pub struct DummyAttacher;

impl Attacher for DummyAttacher {
    type Signal = DummyAttacherSignal;

    fn signal(_pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>> {
        Ok(DummyAttacherSignal)
    }

    async fn signaled() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Dummy attacher signal.
///
/// It does nothing.
pub struct DummyAttacherSignal;

impl AttacherSignal for DummyAttacherSignal {
    async fn send(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::time::Duration;

    use async_io::Timer;
    use futures::{select, FutureExt};

    use super::DummyAttacher;
    use crate::attach::attacher::{Attacher, AttacherSignal};

    #[test]
    fn test_dummy_attacher() {
        let mut exec = futures::executor::LocalPool::new();

        let res = exec.run_until(async {
            let job = async {
                DummyAttacher::signaled().await?;
                DummyAttacher::signal(std::process::id())?.send().await?;
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
