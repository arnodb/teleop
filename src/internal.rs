use std::{fs::File, path::PathBuf};

use sysinfo::{Pid, System};

pub struct AutoDropFile(PathBuf);

impl AutoDropFile {
    pub fn create(path: PathBuf) -> std::io::Result<Self> {
        File::create(&path)?;
        Ok(Self(path))
    }

    pub fn exists(&self) -> Result<bool, std::io::Error> {
        std::fs::exists(&self.0)
    }
}

impl Drop for AutoDropFile {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_file(&self.0).unwrap();
        }
    }
}

pub fn attach_file_path(pid: u32) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let sysinfo_pid = if let Ok(pid) = usize::try_from(pid) {
        Pid::from(pid)
    } else {
        return Err("PID overflows usize".into());
    };
    let s = System::new_all();
    if let Some(process) = s.process(sysinfo_pid) {
        let cwd = process.cwd();
        Ok(cwd
            .ok_or_else(|| -> Box<dyn std::error::Error> {
                "Cannot find process working directory".into()
            })?
            .join(format!(".teleop_attach_{pid}")))
    } else {
        Err("Cannot find process working directory".into())
    }
}
