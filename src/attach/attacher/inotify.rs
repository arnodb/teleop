//! Inotify attacher which creates a file in the process working directory and waits for process to detect it.

use std::path::Path;

use async_io::Async;
use inotify::{Inotify, WatchMask};

use crate::{
    attach::attacher::{Attacher, AttacherSignal},
    internal::{attach_file_path, AutoDropFile},
};

pub struct InotifyAttacher;

impl Attacher for InotifyAttacher {
    type Signal = InotifyAttacherSignal;

    fn signal(pid: u32) -> Result<Self::Signal, Box<dyn std::error::Error>> {
        Ok(InotifyAttacherSignal { pid, file: None })
    }

    async fn signaled() -> Result<(), Box<dyn std::error::Error>> {
        let attach_file_path = attach_file_path(std::process::id())?;
        let parent = attach_file_path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = attach_file_path.file_name().unwrap();
        let inotify = Inotify::init()?;
        inotify.watches().add(parent, WatchMask::CREATE)?;
        let mut async_inotify = Async::new(inotify)?;
        let mut buffer = [0u8; 1024];
        // Detect creation before listening to inotify
        if std::fs::exists(&attach_file_path)? {
            return Ok(());
        }
        loop {
            let read = |inner: &mut Inotify| {
                let events = inner.read_events(&mut buffer)?;
                for event in events {
                    if let Some(name) = event.name {
                        if name == file_name {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            };
            if unsafe { async_inotify.read_with_mut(read) }.await? {
                break;
            };
        }
        Ok(())
    }
}

pub struct InotifyAttacherSignal {
    pid: u32,
    file: Option<AutoDropFile>,
}

impl AttacherSignal for InotifyAttacherSignal {
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

    use super::InotifyAttacher;
    use crate::{
        attach::attacher::tests::test_attacher,
        internal::{attach_file_path, AutoDropFile},
    };

    #[test]
    fn test_inotify_attacher() {
        test_attacher::<InotifyAttacher, _>(async {
            // Create a wrong file
            let mut wrong_attach_file_path = attach_file_path(std::process::id()).unwrap();
            let mut wrong_file_name = wrong_attach_file_path.file_name().unwrap().to_os_string();
            wrong_file_name.push("_wrong");
            wrong_attach_file_path.set_file_name(wrong_file_name);
            let _file = AutoDropFile::create(wrong_attach_file_path);
            // Wait to make sure inotify sees the file
            Timer::after(Duration::from_millis(200)).await;
        });
    }
}
