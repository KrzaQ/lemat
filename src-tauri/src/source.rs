//! Where a board file lives. Today only on this machine; later Lejce will
//! bring SSH hosts, and a remote source will implement the same trait.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

/// What changes when a file changes. Two equal stamps mean the watcher does
/// not need to read the file again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

/// A board file the watcher can poll.
pub trait BoardSource: Send + 'static {
    /// The name to show in the window title.
    fn name(&self) -> String;
    /// The full location, for messages and for remembering the last file.
    fn location(&self) -> String;
    fn stamp(&self) -> io::Result<Stamp>;
    fn read(&self) -> io::Result<String>;
}

pub struct LocalFile {
    path: PathBuf,
}

impl LocalFile {
    pub fn new(path: PathBuf) -> Self {
        // A relative path would change meaning if the working folder changed,
        // and the remembered path must work from any folder.
        let path = std::path::absolute(&path).unwrap_or(path);
        Self { path }
    }
}

impl BoardSource for LocalFile {
    fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.location())
    }

    fn location(&self) -> String {
        self.path.display().to_string()
    }

    fn stamp(&self) -> io::Result<Stamp> {
        let meta = fs::metadata(&self.path)?;
        if meta.is_dir() {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, "folder"));
        }
        Ok(Stamp {
            modified: meta.modified().ok(),
            len: meta.len(),
        })
    }

    fn read(&self) -> io::Result<String> {
        fs::read_to_string(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_becomes_absolute() {
        let file = LocalFile::new(PathBuf::from("board.md"));
        assert!(PathBuf::from(file.location()).is_absolute());
        assert_eq!(file.name(), "board.md");
    }

    #[test]
    fn stamp_changes_with_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.md");
        fs::write(&path, "a").unwrap();
        let file = LocalFile::new(path.clone());
        let first = file.stamp().unwrap();
        fs::write(&path, "ab").unwrap();
        assert_ne!(file.stamp().unwrap(), first);
        assert_eq!(file.read().unwrap(), "ab");
    }

    #[test]
    fn folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = LocalFile::new(dir.path().to_path_buf());
        assert!(file.stamp().is_err());
    }
}
