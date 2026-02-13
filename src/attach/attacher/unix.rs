//! Unix attacher which creates a file in the process working directory and sends a `QUIT` signal
//! to the process.
//!
//! In this post-2025, there is no need to use this:
//!
//! * on `linux`, see `inotify` attacher instead (feature `inotify`)
//! * on `macos`, see `kqueue` attacher instead

use std::future::Future;

use async_signal::{Signal, Signals};
use futures::StreamExt;
use nix::{
    sys::signal::{kill, Signal::SIGQUIT},
    unistd::Pid,
};

use crate::{
    attach::attacher::{Attacher, AttacherSignal},
    internal::{attach_file_path, AutoDropFile},
};

/// UNIX attacher.
///
/// It waits for the `QUIT` signal and checks the presence of the attach file in the working
/// directory.
pub struct UnixAttacher;

impl Attacher for UnixAttacher {
    type Signal = UnixAttacherSignal;

    fn signal(pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>> {
        Ok(UnixAttacherSignal { pid, file: None })
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
                        let attach_file_path = attach_file_path(std::process::id())?;
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

/// UNIX attacher signal.
///
/// It creates the attach file and sends a `QUIT` signal to the target process.
pub struct UnixAttacherSignal {
    pid: u32,
    file: Option<AutoDropFile>,
}

impl AttacherSignal for UnixAttacherSignal {
    async fn send(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Recreate the file if necessary
        if self
            .file
            .as_ref()
            .map(|file| file.exists())
            .transpose()?
            .is_none_or(|exists| !exists)
        {
            self.file = Some(AutoDropFile::create(attach_file_path(self.pid)?)?);
        }
        kill(Pid::from_raw(self.pid as _), SIGQUIT)?;
        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::UnixAttacher;
    use crate::attach::attacher::tests::test_attacher;

    #[test]
    fn test_unix_attacher() {
        test_attacher::<UnixAttacher, _>(async {});
    }
}
