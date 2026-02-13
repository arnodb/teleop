//! Inotify attacher which creates a file in the process working directory and waits for process to detect it.

use std::{
    ops::{Deref, DerefMut},
    os::fd::{AsFd, AsRawFd, BorrowedFd},
    path::Path,
};

use async_io::Async;
use kqueue::{EventFilter, FilterFlag, Watcher};

use crate::{
    attach::attacher::{Attacher, AttacherSignal},
    internal::{attach_file_path, AutoDropFile},
};

struct KqueueWatcherWrapper(Watcher);

impl Deref for KqueueWatcherWrapper {
    type Target = Watcher;

    fn deref(&self) -> &Watcher {
        &self.0
    }
}

impl DerefMut for KqueueWatcherWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsFd for KqueueWatcherWrapper {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.0.as_raw_fd()) }
    }
}

pub struct KqueueAttacher;

impl Attacher for KqueueAttacher {
    type Signal = KqueueAttacherSignal;

    fn signal(pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>> {
        Ok(KqueueAttacherSignal { pid, file: None })
    }

    async fn signaled() -> Result<(), Box<dyn std::error::Error>> {
        let attach_file_path = attach_file_path(std::process::id())?;
        let parent = attach_file_path.parent().unwrap_or_else(|| Path::new("."));
        let mut watcher = KqueueWatcherWrapper(Watcher::new()?);
        watcher.add_filename(parent, EventFilter::EVFILT_VNODE, FilterFlag::NOTE_WRITE)?;
        watcher.watch()?;
        let async_kqueue = Async::new_nonblocking(watcher)?;
        loop {
            if std::fs::exists(&attach_file_path)? {
                return Ok(());
            }
            async_kqueue
                .read_with(|inner| match inner.poll(None) {
                    Some(_) => Ok(()),
                    None => Err(std::io::ErrorKind::WouldBlock.into()),
                })
                .await?;
        }
    }
}

pub struct KqueueAttacherSignal {
    pid: u32,
    file: Option<AutoDropFile>,
}

impl AttacherSignal for KqueueAttacherSignal {
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
        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::time::Duration;

    use async_io::Timer;

    use super::KqueueAttacher;
    use crate::{
        attach::attacher::tests::test_attacher,
        internal::{attach_file_path, AutoDropFile},
    };

    #[test]
    fn test_kqueue_attacher() {
        test_attacher::<KqueueAttacher, _>(async {
            // Create a wrong file
            let mut wrong_attach_file_path = attach_file_path(std::process::id()).unwrap();
            let mut wrong_file_name = wrong_attach_file_path.file_name().unwrap().to_os_string();
            wrong_file_name.push("_wrong");
            wrong_attach_file_path.set_file_name(wrong_file_name);
            let _file = AutoDropFile::create(wrong_attach_file_path);
            // Wait to make sure kqueue sees the file
            Timer::after(Duration::from_millis(200)).await;
        });
    }
}
