mod board;
mod recent;
mod source;
mod watch;

use std::ffi::OsString;
use std::path::PathBuf;

use tauri::menu::{Menu, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::DialogExt;

use board::{Board, Snapshot};

const OPEN: &str = "open";

/// The web view asks once on load, because it may miss events sent before
/// its listener was ready.
#[tauri::command]
fn board_current(board: tauri::State<'_, Board>) -> Snapshot {
    board.snapshot()
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
        Menu::with_items(app, &[&lemat, &file, &edit, &window])
    }
    #[cfg(not(target_os = "macos"))]
    Menu::with_items(app, &[&file, &edit, &window])
}

fn pick_file(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .add_filter("Markdown", &["md", "markdown", "txt"])
        .pick_file(move |picked| {
            if let Some(path) = picked.and_then(|p| p.into_path().ok()) {
                board::open_local(&app, path);
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Board::default())
        .menu(menu)
        .on_menu_event(|app, event| {
            if event.id() == OPEN {
                pick_file(app);
            }
        })
        .invoke_handler(tauri::generate_handler![board_current])
        .setup(|app| {
            if let Some(path) = first_board(app.handle()) {
                board::open_local(app.handle(), path);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Lemat");
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
