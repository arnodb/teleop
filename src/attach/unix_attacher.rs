use std::{future::Future, path::PathBuf};

use async_signal::{Signal, Signals};
use futures::StreamExt;
use nix::{
    sys::signal::{kill, Signal::SIGQUIT},
    unistd::Pid,
};

use crate::{
    attach::{Attacher, AttacherSignal},
    internal::AutoDropFile,
};

pub struct UnixAttacher;

impl Attacher for UnixAttacher {
    type Signal = UnixAttacherSignal;

    fn signal(pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>> {
        Ok(UnixAttacherSignal {
            pid,
            _file: AutoDropFile::create(attach_file_path(pid))?,
        })
    }

    fn signaled() -> impl Future<Output = Result<(), Box<dyn std::error::Error>>> {
        // It is important to keep this in the synchronous part in order to ensure the listening
        // process is ready to accept attachment requests even if the future is not awaited.
        //
        // Nevertheless, the error will only be raised if the future is awaited.
        let signals = Signals::new([Signal::Quit]);

        async move {
            let mut signals = signals?;

            while let Some(signal) = signals.next().await {
                if let Ok(signal) = signal {
                    if signal == Signal::Quit {
                        let attach_file_path = attach_file_path(std::process::id());
                        if attach_file_path.exists() {
                            break;
                        }
                    }
                }
            }

            Ok(())
        }
    }
}

pub struct UnixAttacherSignal {
    pid: u32,
    _file: AutoDropFile,
}

impl AttacherSignal for UnixAttacherSignal {
    async fn send(&self) -> Result<(), Box<dyn std::error::Error>> {
        kill(Pid::from_raw(self.pid as _), SIGQUIT)?;
        Ok(())
    }
}

fn attach_file_path(pid: u32) -> PathBuf {
    let mut path = PathBuf::new();
    path.push("/proc");
    path.push(pid.to_string());
    path.push("cwd");
    path.push(format!(".teleop_attach_{pid}"));
    path
}
