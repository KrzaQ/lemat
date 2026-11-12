//! The project's settings: `lemat.toml` next to a board file, or a
//! project file of its own that Lemat opens (`studies.toml`).
//!
//! ```toml
//! engine = "claude"          # claude, pi
//! model = "sonnet"
//! effort = "medium"          # optional
//! host = "server"            # an ssh destination; leave out for this machine
//! folder = "~/notes/math"    # the project folder on that machine
//! board = "boards/limit.md"  # a project file: the board, inside the folder
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use lejce::{Engine, Host};
use serde::Deserialize;

pub const FILE: &str = "lemat.toml";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub engine: String,
    pub model: String,
    pub effort: Option<String>,
    pub host: Option<String>,
    pub folder: Option<String>,
    /// The board a project file opens, relative to `folder`.
    pub board: Option<String>,
    /// MCP servers come in step 3 of the design. Kept here so that a file
    /// written for that step still loads.
    #[serde(rename = "mcp")]
    pub mcp: Option<toml::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            engine: "claude".into(),
            model: "sonnet".into(),
            effort: None,
            host: None,
            folder: None,
            board: None,
            mcp: None,
        }
    }
}

impl Settings {
    /// The settings for a board file. No file means the defaults: Claude on
    /// this machine, in the board's folder.
    pub fn for_board(board: &Path) -> Result<Self, String> {
        let path = board.with_file_name(FILE);
        match fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let settings: Settings = toml::from_str(text).map_err(|e| e.message().to_string())?;
        settings.engine()?;
        Ok(settings)
    }

    pub fn engine(&self) -> Result<Engine, String> {
        Engine::parse(&self.engine).ok_or_else(|| format!("nieznany silnik: {}", self.engine))
    }

    /// Where the engine runs. On this machine the folder defaults to the
    /// board's folder, so that the agent reaches the board file.
    pub fn host(&self, board: &Path) -> Host {
        match &self.host {
            Some(destination) => Host::Ssh {
                destination: destination.clone(),
                folder: self.folder.clone(),
            },
            None => Host::Local {
                folder: Some(match &self.folder {
                    Some(folder) => expand_home(folder),
                    None => board.parent().map(Path::to_path_buf).unwrap_or_default(),
                }),
            },
        }
    }
}

/// "~/notes" on this machine. A remote folder stays as it is: the remote
/// shell expands it.
pub fn expand_home(folder: &str) -> PathBuf {
    match (folder.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(folder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Settings::parse("").unwrap(), Settings::default());
    }

    #[test]
    fn reads_the_design_example() {
        let text = r#"
            engine = "pi"
            model = "opus"
            host = "server"
            folder = "~/notes/math"

            [[mcp]]
            name = "example"
            url = "http://localhost:8123/mcp"
            forward = true
        "#;
        let settings = Settings::parse(text).unwrap();
        assert_eq!(settings.engine().unwrap(), Engine::Pi);
        assert_eq!(settings.model, "opus");
        assert_eq!(
            settings.host(Path::new("/b/board.md")),
            Host::Ssh {
                destination: "server".into(),
                folder: Some("~/notes/math".into())
            }
        );
    }

    #[test]
    fn local_folder_defaults_to_the_board_folder() {
        let host = Settings::default().host(Path::new("/notes/math/board.md"));
        assert_eq!(
            host,
            Host::Local {
                folder: Some("/notes/math".into())
            }
        );
    }

    #[test]
    fn rejects_unknown_engine_and_typos() {
        assert!(Settings::parse("engine = \"gpt\"").is_err());
        assert!(Settings::parse("modle = \"opus\"").is_err());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let board = dir.path().join("board.md");
        assert_eq!(Settings::for_board(&board).unwrap(), Settings::default());
        fs::write(dir.path().join(FILE), "model = \"opus\"").unwrap();
        assert_eq!(Settings::for_board(&board).unwrap().model, "opus");
    }
}
