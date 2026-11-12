//! What Lemat opens: a board file on this machine, or a project file that
//! names a host, a folder and a board. A project on another host keeps its
//! boards and history there, and Lemat reaches them over SSH.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use lejce::Host;

use crate::files::{Files, Local, Ssh};
use crate::history;
use crate::settings::{expand_home, Settings};

/// The file the user opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A Markdown board, with `lemat.toml` next to it if any.
    Board(PathBuf),
    /// A project file (`.toml`).
    Project(PathBuf),
}

impl Target {
    pub fn from_path(path: PathBuf) -> Target {
        let path = std::path::absolute(&path).unwrap_or(path);
        if path.extension().is_some_and(|e| e == "toml") {
            Target::Project(path)
        } else {
            Target::Board(path)
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Target::Board(path) | Target::Project(path) => path,
        }
    }
}

pub struct Project {
    pub source: Target,
    pub settings: Settings,
    /// Where the boards and the history live.
    pub files: Arc<dyn Files>,
    /// Where the engine runs, for Lejce.
    pub engine: Host,
}

/// The open board of a project.
#[derive(Clone)]
pub struct Place {
    pub project: Arc<Project>,
    pub board: PathBuf,
}

impl Place {
    pub fn label(&self) -> String {
        self.project.files.label(&self.board)
    }

    /// The window title: the board's name, and the host if it is remote.
    pub fn title(&self) -> String {
        let name = self.board.file_name().unwrap_or_default().to_string_lossy();
        match self.project.files.host() {
            Some(host) => format!("{name} — {host} — Lemat"),
            None => format!("{name} — Lemat"),
        }
    }

    pub fn history(&self) -> PathBuf {
        history::path_for(&self.board)
    }

    pub fn with_board(&self, board: PathBuf) -> Place {
        Place {
            project: self.project.clone(),
            board,
        }
    }
}

/// A Markdown file inside a folder, as a relative path: board paths from
/// settings and from the agent must not reach other files.
pub fn inside(file: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(file);
    let plain = path.components().all(|c| matches!(c, Component::Normal(_)));
    if !plain || path.extension().is_none_or(|e| e != "md") {
        return Err(format!(
            "{file}: a board is a .md file inside the project folder, as a relative path."
        ));
    }
    Ok(path)
}

/// Opens the target. For a project on another host this connects over
/// SSH, so it may take seconds; call it off the main thread.
pub fn open(target: &Target) -> Result<Place, String> {
    match target {
        Target::Board(board) => {
            let settings = Settings::for_board(board).map_err(|e| format!("Ustawienia: {e}"))?;
            let engine = settings.host(board);
            let project = Project {
                source: target.clone(),
                settings,
                files: Arc::new(Local),
                engine,
            };
            Ok(Place {
                project: Arc::new(project),
                board: board.clone(),
            })
        }
        Target::Project(file) => {
            let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
            let settings = Settings::parse(&text)
                .map_err(|e| format!("Ustawienia {}: {e}", file.display()))?;
            let board = inside(settings.board.as_deref().unwrap_or("board.md"))?;
            let (files, folder, engine): (Arc<dyn Files>, PathBuf, Host) = match &settings.host {
                Some(host) => {
                    let wanted = settings.folder.as_deref().unwrap_or("~");
                    let (ssh, folder) = Ssh::connect(host, wanted).map_err(|e| match e.kind() {
                        std::io::ErrorKind::NotFound => {
                            format!("Na {host} nie ma folderu {wanted}.")
                        }
                        _ => format!("Nie można połączyć się z {host}: {e}"),
                    })?;
                    let engine = Host::Ssh {
                        destination: host.clone(),
                        folder: Some(folder.to_string_lossy().into_owned()),
                    };
                    (Arc::new(ssh), folder, engine)
                }
                None => {
                    let folder = match &settings.folder {
                        Some(folder) => expand_home(folder),
                        None => file.parent().map(Path::to_path_buf).unwrap_or_default(),
                    };
                    if !folder.is_dir() {
                        return Err(format!("Nie ma folderu {}.", folder.display()));
                    }
                    let engine = Host::Local {
                        folder: Some(folder.clone()),
                    };
                    (Arc::new(Local), folder, engine)
                }
            };
            let project = Project {
                source: target.clone(),
                settings,
                files,
                engine,
            };
            Ok(Place {
                project: Arc::new(project),
                board: folder.join(board),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_is_a_project_and_the_rest_a_board() {
        assert!(matches!(
            Target::from_path("/n/studia.toml".into()),
            Target::Project(_)
        ));
        assert!(matches!(
            Target::from_path("/n/granica.md".into()),
            Target::Board(_)
        ));
    }

    #[test]
    fn board_paths_stay_inside() {
        assert!(inside("lemat/granica.md").is_ok());
        for bad in ["../x.md", "/tmp/x.md", "notes.txt", "a/../../x.md"] {
            assert!(inside(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn local_project_file_opens_its_board() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("studia.toml");
        fs::write(&file, "model = \"opus\"\nboard = \"lemat/granica.md\"\n").unwrap();
        let place = open(&Target::Project(file)).unwrap();
        assert_eq!(place.board, dir.path().join("lemat/granica.md"));
        assert_eq!(place.history(), dir.path().join("lemat/lemat.jsonl"));
        assert_eq!(place.project.settings.model, "opus");
        assert_eq!(
            place.project.engine,
            Host::Local {
                folder: Some(dir.path().to_path_buf())
            }
        );
        assert_eq!(place.title(), "granica.md — Lemat");
    }

    #[test]
    fn project_file_with_a_missing_folder_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("p.toml");
        fs::write(&file, "folder = \"/nonexistent/lemat-test\"\n").unwrap();
        let error = open(&Target::Project(file)).err().unwrap();
        assert!(error.starts_with("Nie ma folderu"), "{error}");
    }

    #[test]
    fn board_file_keeps_its_settings_next_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let board = dir.path().join("b.md");
        let place = open(&Target::Board(board.clone())).unwrap();
        assert_eq!(place.board, board);
        assert_eq!(place.project.files.host(), None);
    }
}
