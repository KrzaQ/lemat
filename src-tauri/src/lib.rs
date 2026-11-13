mod board;
mod board_tools;
mod chat;
mod document;
mod files;
mod history;
mod mcp;
mod project;
mod recent;
mod settings;
mod watch;

use std::ffi::OsString;
use std::path::PathBuf;

use tauri::menu::{Menu, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;

use board::{Board, Snapshot};
use chat::Chat;

const OPEN: &str = "open";
const STOP: &str = "stop";

/// The board tools reach the open board through the app.
struct Desk(AppHandle);

impl board_tools::Desk for Desk {
    fn current(&self) -> Option<project::Place> {
        self.0.state::<Board>().place()
    }

    fn open(&self, place: project::Place) {
        board::open_place(&self.0, place);
    }
}

/// The web view asks once on load, because it may miss events sent before
/// its listener was ready.
#[tauri::command]
fn board_current(board: tauri::State<'_, Board>) -> Snapshot {
    board.snapshot()
}

fn open_board(board: &Board) -> Result<project::Place, String> {
    board
        .place()
        .ok_or_else(|| "Najpierw otwórz tablicę albo projekt.".into())
}

#[tauri::command]
fn chat_state(board: tauri::State<'_, Board>, chat: tauri::State<'_, Chat>) -> Option<chat::State> {
    open_board(&board).ok().map(|place| chat.state(&place))
}

#[tauri::command]
fn chat_send(
    text: String,
    board: tauri::State<'_, Board>,
    chat: tauri::State<'_, Chat>,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    chat.send(open_board(&board)?, text.to_string());
    Ok(())
}

/// Saves the engine and model in the project's settings. The next message
/// goes to them: a new engine gets the history in a new session.
#[tauri::command]
fn chat_set_engine(
    engine: String,
    model: String,
    board: tauri::State<'_, Board>,
) -> Result<(), String> {
    let place = open_board(&board)?;
    settings::save_engine(&place.project.settings_file(), &engine, &model)
}

/// Model names to offer for an engine. For pi this asks pi on the engine's
/// machine, which may take a moment over SSH.
#[tauri::command]
async fn chat_models(
    engine: String,
    board: tauri::State<'_, Board>,
) -> Result<Vec<String>, String> {
    let place = open_board(&board)?;
    let settings = chat::settings_now(&place.project, &place.board)?;
    let host = chat::engine_host(&place, &settings);
    tauri::async_runtime::spawn_blocking(move || chat::models(&engine, &host))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn chat_stop(chat: tauri::State<'_, Chat>) {
    chat.stop();
}

fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let open = MenuItemBuilder::with_id(OPEN, "Otwórz…")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let file = SubmenuBuilder::new(app, "Plik")
        .item(&open)
        .separator()
        .close_window();
    #[cfg(not(target_os = "macos"))]
    let file = file.quit();
    let file = file.build()?;
    let edit = SubmenuBuilder::new(app, "Edycja")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    // Esc in the window stops too; the menu works wherever the focus is.
    let stop = MenuItemBuilder::with_id(STOP, "Zatrzymaj agenta")
        .accelerator("CmdOrCtrl+.")
        .build(app)?;
    let conversation = SubmenuBuilder::new(app, "Rozmowa").item(&stop).build()?;
    let window = SubmenuBuilder::new(app, "Okno")
        .minimize()
        .fullscreen()
        .build()?;

    // macOS puts the first submenu under the program's name.
    #[cfg(target_os = "macos")]
    {
        let lemat = SubmenuBuilder::new(app, "Lemat")
            .about(None)
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?;
        Menu::with_items(app, &[&lemat, &file, &edit, &conversation, &window])
    }
    #[cfg(not(target_os = "macos"))]
    Menu::with_items(app, &[&file, &edit, &conversation, &window])
}

fn pick_file(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .add_filter("Tablica albo projekt", &["md", "markdown", "txt", "toml"])
        .pick_file(move |picked| {
            if let Some(path) = picked.and_then(|p| p.into_path().ok()) {
                board::open(&app, project::Target::from_path(path));
            }
        });
}

/// The command line wins over the last file: `lemat path/to/board.md`.
fn first_board(app: &AppHandle) -> Option<PathBuf> {
    board_argument(std::env::args_os().skip(1)).or_else(|| board::last_board(app))
}

/// The first argument that is not an option. `tauri dev` may pass a `--`
/// before the program's own arguments.
fn board_argument(args: impl Iterator<Item = OsString>) -> Option<PathBuf> {
    args.map(PathBuf::from)
        .find(|arg| !arg.to_string_lossy().starts_with('-'))
}

/// An app started from Finder gets a short PATH, without the places where
/// the engines (claude, pi) and their tools (node) usually live.
fn extend_path() {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if let Ok(joined) = std::env::join_paths(dirs) {
        std::env::set_var("PATH", joined);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    extend_path();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Board::default())
        .menu(menu)
        .on_menu_event(|app, event| {
            if event.id() == OPEN {
                pick_file(app);
            } else if event.id() == STOP {
                app.state::<Chat>().stop();
            }
        })
        .invoke_handler(tauri::generate_handler![
            board_current,
            chat_state,
            chat_send,
            chat_stop,
            chat_set_engine,
            chat_models
        ])
        .setup(|app| {
            let tools = board_tools::BoardTools::new(Desk(app.handle().clone()));
            let server = mcp::serve("lemat", tools)?;
            app.manage(Chat::start(app.handle(), server.url));
            if let Some(path) = first_board(app.handle()) {
                board::open(app.handle(), project::Target::from_path(path));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Lemat")
        .run(|_, event| {
            // Each engine runs in its own process group, so it outlives
            // Lemat unless Lemat stops it. Over SSH it would keep running on
            // the other machine.
            if let tauri::RunEvent::Exit = event {
                lejce::cancel_all();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = OsString> {
        list.iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn board_argument_skips_options() {
        assert_eq!(board_argument(args(&[])), None);
        assert_eq!(
            board_argument(args(&["--", "a.md"])),
            Some(PathBuf::from("a.md"))
        );
        assert_eq!(board_argument(args(&["b.md"])), Some(PathBuf::from("b.md")));
    }
}
