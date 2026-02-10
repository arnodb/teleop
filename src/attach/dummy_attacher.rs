//! Dummy attacher which listens immediately.

use crate::attach::{Attacher, AttacherSignal};

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
    use futures::FutureExt;
    use futures_lite::future::or;

    use super::DummyAttacher;
    use crate::attach::{Attacher, AttacherSignal};

    #[test]
    fn test_dummy_attacher() {
        let mut exec = futures::executor::LocalPool::new();

        let res = exec.run_until(or(
            async {
                DummyAttacher::signaled().await?;
                DummyAttacher::signal(std::process::id())?.send().await?;
                Ok::<_, Box<dyn std::error::Error>>(())
            },
            Timer::after(Duration::from_secs(5)).then(async |_| Err("Test timeout".into())),
        ));

        exec.run();

        res.unwrap();
    }
}
