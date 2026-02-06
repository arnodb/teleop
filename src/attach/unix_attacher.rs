use std::{future::Future, path::PathBuf};

use async_signal::{Signal, Signals};
use futures::StreamExt;

use crate::{attach::Attacher, internal::AutoDropFile};

pub struct UnixAttacher;

impl Attacher for UnixAttacher {
    type SignalGuard = AutoDropFile;

    async fn send_signal(pid: u32) -> Result<Self::SignalGuard, Box<dyn std::error::Error>> {
        let attach_file: AutoDropFile = AutoDropFile::create(attach_file_path(pid))?;
        Ok(attach_file)
    }

    fn await_signal() -> impl Future<Output = Result<(), Box<dyn std::error::Error>>> {
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

fn attach_file_path(pid: u32) -> PathBuf {
    let mut path = PathBuf::new();
    path.push("/proc");
    path.push(pid.to_string());
    path.push("cwd");
    path.push(format!(".teleop_attach_{pid}"));
    path
}
