//! Remembers the last board file between runs.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Debug, PartialEq)]
pub struct Recent {
    pub last_board: Option<PathBuf>,
}

impl Recent {
    /// A missing or broken file means a first run: start with nothing.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/recent.json");
        assert_eq!(Recent::load(&path), Recent::default());
        let recent = Recent {
            last_board: Some(PathBuf::from("/notes/board.md")),
        };
        recent.save(&path).unwrap();
        assert_eq!(Recent::load(&path), recent);
    }

    #[test]
    fn broken_file_gives_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Recent::load(&path), Recent::default());
    }
}
