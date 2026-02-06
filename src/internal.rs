use std::{fs::File, path::PathBuf};

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
