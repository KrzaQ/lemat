//! The open board: which file it shows, its last text, and the thread that
//! watches it and sends each change to the web view.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::project::{self, Place, Target};
use crate::recent::Recent;
use crate::watch::Watch;

/// How long to wait before trying an unreachable project again.
const RETRY: Duration = Duration::from_secs(5);

/// The event the web view listens to.
pub const EVENT: &str = "board";

/// What the board shows. `text` stays at the last good text while `error`
/// says why the file cannot be read now.
#[derive(Clone, Default, Serialize, Debug)]
pub struct Snapshot {
    /// Grows with each change, so that the web view can drop an answer
    /// older than an event it already drew.
    pub version: u64,
    /// The board's place as the user reads it, "server:/notes/board.md"
    /// for a remote one.
    pub path: Option<String>,
    pub text: Option<String>,
    pub error: Option<String>,
    /// The project is open: the conversation can start.
    pub ready: bool,
}

#[derive(Default)]
struct State {
    /// Each open gets a new number. A watcher thread stops when the number
    /// is no longer its own. It lives under the same lock as the snapshot,
    /// so an old thread cannot write over the file opened after it.
    generation: u64,
    snapshot: Snapshot,
    /// None until the project is open (an SSH project connects first).
    place: Option<Place>,
}

#[derive(Default)]
pub struct Board(Mutex<State>);

impl Board {
    pub fn snapshot(&self) -> Snapshot {
        self.0.lock().unwrap().snapshot.clone()
    }

    pub fn place(&self) -> Option<Place> {
        self.0.lock().unwrap().place.clone()
    }
}

/// Opens a board file or a project file.
pub fn open(app: &AppHandle, target: Target) {
    remember(app, target.path().to_path_buf());
    let label = target.path().display().to_string();
    start(app, label, move || project::open(&target));
}

/// Opens another board of the open project: the board tools do this.
pub fn open_place(app: &AppHandle, place: Place) {
    if let Target::Board(_) = place.project.source {
        remember(app, place.board.clone());
    }
    start(app, place.label(), move || Ok(place.clone()));
}

/// Changes the snapshot if this thread's open is still the current one.
/// Returns false when a newer open took over.
fn update(app: &AppHandle, generation: u64, change: impl FnOnce(&mut State)) -> bool {
    let board = app.state::<Board>();
    let snapshot = {
        let mut state = board.0.lock().unwrap();
        if state.generation != generation {
            return false;
        }
        change(&mut state);
        state.snapshot.version += 1;
        state.snapshot.clone()
    };
    let _ = app.emit(EVENT, snapshot);
    true
}

fn start(
    app: &AppHandle,
    label: String,
    mut resolve: impl FnMut() -> Result<Place, String> + Send + 'static,
) {
    let generation = {
        let board = app.state::<Board>();
        let mut state = board.0.lock().unwrap();
        state.generation += 1;
        state.place = None;
        state.snapshot = Snapshot {
            version: state.snapshot.version + 1,
            path: Some(label),
            error: Some("Otwieram…".into()),
            ..Snapshot::default()
        };
        state.generation
    };
    let app = app.clone();
    std::thread::spawn(move || {
        // A project on another host may be unreachable for a while: say
        // why, and try again.
        let place = loop {
            match resolve() {
                Ok(place) => break place,
                Err(error) => {
                    if !update(&app, generation, |s| s.snapshot.error = Some(error)) {
                        return;
                    }
                    std::thread::sleep(RETRY);
                }
            }
        };
        let ready = update(&app, generation, |s| {
            s.snapshot.path = Some(place.label());
            s.snapshot.error = None;
            s.snapshot.ready = true;
            s.place = Some(place.clone());
        });
        if !ready {
            return;
        }
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_title(&place.title());
        }
        let files = place.project.files.clone();
        let mut watch = Watch::new(files.clone(), place.board.clone());
        loop {
            let current = match watch.poll() {
                Some(change) => update(&app, generation, |s| match change {
                    Ok(text) => {
                        s.snapshot.text = Some(text);
                        s.snapshot.error = None;
                    }
                    Err(error) => s.snapshot.error = Some(error),
                }),
                None => app.state::<Board>().0.lock().unwrap().generation == generation,
            };
            if !current {
                return;
            }
            std::thread::sleep(files.poll());
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
