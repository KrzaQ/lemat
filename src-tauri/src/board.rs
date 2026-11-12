//! The open board: which file it shows, its last text, and the thread that
//! watches it and sends each change to the web view.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::recent::Recent;
use crate::source::{BoardSource, LocalFile};
use crate::watch::Watch;

/// Short enough that an edit in another editor shows up at once, long
/// enough that the poll costs nothing.
const POLL: Duration = Duration::from_millis(250);

/// The event the web view listens to.
pub const EVENT: &str = "board";

/// What the board shows. `text` stays at the last good text while `error`
/// says why the file cannot be read now.
#[derive(Clone, Default, Serialize, Debug)]
pub struct Snapshot {
    /// Grows with each change, so that the web view can drop an answer
    /// older than an event it already drew.
    pub version: u64,
    pub path: Option<String>,
    pub text: Option<String>,
    pub error: Option<String>,
}

#[derive(Default)]
struct State {
    /// Each open gets a new number. A watcher thread stops when the number
    /// is no longer its own. It lives under the same lock as the snapshot,
    /// so an old thread cannot write over the file opened after it.
    generation: u64,
    snapshot: Snapshot,
}

#[derive(Default)]
pub struct Board(Mutex<State>);

impl Board {
    pub fn snapshot(&self) -> Snapshot {
        self.0.lock().unwrap().snapshot.clone()
    }
}

pub fn open_local(app: &AppHandle, path: PathBuf) {
    open(app, LocalFile::new(path));
}

fn open<S: BoardSource>(app: &AppHandle, source: S) {
    let location = source.location();
    let board = app.state::<Board>();
    let generation = {
        let mut state = board.0.lock().unwrap();
        state.generation += 1;
        state.snapshot = Snapshot {
            version: state.snapshot.version + 1,
            path: Some(location.clone()),
            ..Snapshot::default()
        };
        state.generation
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(&format!("{} — Lemat", source.name()));
    }
    remember(app, PathBuf::from(location));

    let app = app.clone();
    std::thread::spawn(move || {
        let mut watch = Watch::new(source);
        loop {
            let change = watch.poll();
            let board = app.state::<Board>();
            let snapshot = {
                let mut state = board.0.lock().unwrap();
                if state.generation != generation {
                    return;
                }
                change.map(|change| {
                    let current = &mut state.snapshot;
                    current.version += 1;
                    match change {
                        Ok(text) => {
                            current.text = Some(text);
                            current.error = None;
                        }
                        Err(error) => current.error = Some(error),
                    }
                    current.clone()
                })
            };
            if let Some(snapshot) = snapshot {
                let _ = app.emit(EVENT, snapshot);
            }
            std::thread::sleep(POLL);
        }
    });
}

fn recent_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("recent.json"))
}

pub fn last_board(app: &AppHandle) -> Option<PathBuf> {
    Recent::load(&recent_path(app)?).last_board
}

fn remember(app: &AppHandle, path: PathBuf) {
    let Some(file) = recent_path(app) else { return };
    let recent = Recent {
        last_board: Some(path),
    };
    if let Err(e) = recent.save(&file) {
        eprintln!("lemat: cannot save {}: {e}", file.display());
    }
}
