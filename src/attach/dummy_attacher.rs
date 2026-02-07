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
