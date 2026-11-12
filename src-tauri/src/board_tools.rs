//! The board's MCP tools: the agent writes steps and marks through them
//! instead of editing the board file. `docs/board-format.md` describes the
//! file they write.

use std::io;
use std::path::Path;

use serde_json::{json, Value};

use crate::document::{self, Document, State};
use crate::mcp::Toolbox;
use crate::project::{inside, Place};

/// Where the tools find the open board, and how they open another. The
/// board may be on another machine: the tools reach it through the
/// project's files.
pub trait Desk: Send + Sync + 'static {
    fn current(&self) -> Option<Place>;
    fn open(&self, place: Place);
}

pub struct BoardTools<D> {
    desk: D,
}

impl<D: Desk> BoardTools<D> {
    pub fn new(desk: D) -> Self {
        Self { desk }
    }

    fn place(&self) -> Result<Place, String> {
        self.desk
            .current()
            .ok_or_else(|| "No board is open in Lemat.".into())
    }

    fn read(&self) -> Result<(Place, String), String> {
        let place = self.place()?;
        let files = &place.project.files;
        let text = files
            .read(&place.board)
            .map_err(|e| format!("{}: {e}", files.label(&place.board)))?;
        Ok((place, text))
    }

    fn change(
        &self,
        edit: impl FnOnce(&mut Document) -> Result<String, String>,
    ) -> Result<String, String> {
        let place = self.place()?;
        let files = &place.project.files;
        // A board that does not exist yet starts empty: a project file may
        // name a board before the first step.
        let text = match files.read(&place.board) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("{}: {e}", files.label(&place.board))),
        };
        let mut document = Document::parse(&text);
        let reply = edit(&mut document)?;
        files
            .write(&place.board, &document.render())
            .map_err(|e| format!("{}: {e}", files.label(&place.board)))?;
        Ok(reply)
    }

    fn show(path: &Path, text: &str) -> String {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        format!("Board file: {name}\n\n{text}")
    }

    fn board_read(&self) -> Result<String, String> {
        let (place, text) = self.read()?;
        Ok(Self::show(&place.board, &text))
    }

    fn board_open(&self, args: &Value) -> Result<String, String> {
        let file = text(args, "file")?;
        let current = self.place()?;
        let folder = current.board.parent().unwrap_or(Path::new(""));
        let path = folder.join(inside(file)?);
        let files = &current.project.files;
        if !files.exists(&path).map_err(|e| e.to_string())? {
            let (Some(title), Some(task)) = (args["title"].as_str(), args["task"].as_str()) else {
                return Err(format!(
                    "{file} does not exist. Give a title and a task to start a new board."
                ));
            };
            files
                .write(&path, &document::new_board(title, task))
                .map_err(|e| e.to_string())?;
        }
        let text = files.read(&path).map_err(|e| format!("{file}: {e}"))?;
        self.desk.open(current.with_board(path.clone()));
        Ok(Self::show(&path, &text))
    }

    fn board_step(&self, args: &Value) -> Result<String, String> {
        let note = text(args, "note")?;
        let number = number(args, "number")?;
        let latex = args["latex"].as_str();
        self.change(|document| {
            let number = document.set_step(number, note, latex);
            Ok(format!("Step {number} is on the board."))
        })
    }

    fn board_mark(&self, args: &Value) -> Result<String, String> {
        let number = number(args, "number")?.ok_or("Give the step's number.")?;
        let state = State::parse(text(args, "state")?).ok_or("state is checked or wrong.")?;
        let note = text(args, "note")?;
        self.change(|document| {
            document.set_mark(number, state, note)?;
            Ok(format!("Step {number} is marked."))
        })
    }
}

fn text<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args[name]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{name} is missing."))
}

fn number(args: &Value, name: &str) -> Result<Option<u32>, String> {
    match &args[name] {
        Value::Null => Ok(None),
        value => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .map(Some)
            .ok_or_else(|| format!("{name} is a step number from 1.")),
    }
}

impl<D: Desk> Toolbox for BoardTools<D> {
    fn definitions(&self) -> Value {
        json!([
            {
                "name": "board_read",
                "description": "The board as it is now: the Markdown file the user sees, \
                    with its steps and marks. Read it before you change steps: the user \
                    may have edited it by hand.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "board_step",
                "description": "Write a numbered step on the board: a short note in words \
                    and, if the step has one, its formula. Without a number the step goes \
                    after the last one. With the number of an existing step it replaces \
                    that step and removes its mark.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "note": { "type": "string", "description": "The step in words, in the user's language. Markdown; inline formulas in $...$." },
                        "latex": { "type": "string", "description": "The step's formula in LaTeX, shown on its own line. Without $$." },
                        "number": { "type": "integer", "minimum": 1, "description": "Replace this step. Leave out to add a new step." }
                    },
                    "required": ["note"]
                }
            },
            {
                "name": "board_mark",
                "description": "Mark a step after you checked it: checked if it is right, \
                    wrong with one sentence about the mistake. A new mark replaces the old.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "number": { "type": "integer", "minimum": 1 },
                        "state": { "type": "string", "enum": ["checked", "wrong"] },
                        "note": { "type": "string", "description": "One sentence, in the user's language. For a wrong step: what is wrong, not the fixed step." }
                    },
                    "required": ["number", "state", "note"]
                }
            },
            {
                "name": "board_open",
                "description": "Open another board in the window: an existing Markdown file, \
                    or a new task when the file does not exist yet (then give title and task).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "A .md file, relative to the current board's folder." },
                        "title": { "type": "string", "description": "For a new board: its title." },
                        "task": { "type": "string", "description": "For a new board: the task, in Markdown with LaTeX." }
                    },
                    "required": ["file"]
                }
            }
        ])
    }

    fn call(&self, name: &str, arguments: &Value) -> Result<String, String> {
        match name {
            "board_read" => self.board_read(),
            "board_step" => self.board_step(arguments),
            "board_mark" => self.board_mark(arguments),
            "board_open" => self.board_open(arguments),
            _ => Err(format!("No tool {name}.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{open, Target};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct FakeDesk(Mutex<Option<Place>>);

    impl Desk for FakeDesk {
        fn current(&self) -> Option<Place> {
            self.0.lock().unwrap().clone()
        }
        fn open(&self, place: Place) {
            *self.0.lock().unwrap() = Some(place);
        }
    }

    fn tools(dir: &Path) -> BoardTools<FakeDesk> {
        let board = dir.join("granica.md");
        fs::write(&board, "# Granica\n\nOblicz.\n").unwrap();
        let place = open(&Target::Board(board)).unwrap();
        BoardTools::new(FakeDesk(Mutex::new(Some(place))))
    }

    fn current(tools: &BoardTools<FakeDesk>) -> PathBuf {
        tools.desk.current().unwrap().board
    }

    #[test]
    fn step_and_mark_write_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tools(dir.path());
        let reply = tools
            .call(
                "board_step",
                &json!({ "note": "Dzielę.", "latex": "\\frac{3}{2}" }),
            )
            .unwrap();
        assert_eq!(reply, "Step 1 is on the board.");
        tools
            .call(
                "board_mark",
                &json!({ "number": 1, "state": "wrong", "note": "Źle." }),
            )
            .unwrap();
        let text = fs::read_to_string(dir.path().join("granica.md")).unwrap();
        assert!(text.contains("<!-- step 1 -->\n**1.** Dzielę.\n\n$$\n\\frac{3}{2}\n$$"));
        assert!(text.contains("<!-- mark wrong -->\n> ⚠ Źle."));
        let read = tools.call("board_read", &json!({})).unwrap();
        assert!(read.starts_with("Board file: granica.md\n\n# Granica"));
    }

    #[test]
    fn first_step_creates_a_missing_board() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tools(dir.path());
        fs::remove_file(dir.path().join("granica.md")).unwrap();
        tools.call("board_step", &json!({ "note": "a" })).unwrap();
        let text = fs::read_to_string(dir.path().join("granica.md")).unwrap();
        assert_eq!(text, "<!-- step 1 -->\n**1.** a\n");
    }

    #[test]
    fn bad_arguments_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tools(dir.path());
        assert!(tools.call("board_step", &json!({})).is_err());
        assert!(tools
            .call(
                "board_mark",
                &json!({ "number": 1, "state": "checked", "note": "x" })
            )
            .is_err());
        assert!(tools
            .call("board_step", &json!({ "note": "x", "number": 0 }))
            .is_err());
        assert!(tools.call("board_x", &json!({})).is_err());
    }

    #[test]
    fn open_creates_a_new_board_in_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tools(dir.path());
        assert!(tools
            .call("board_open", &json!({ "file": "nowa.md" }))
            .is_err());
        let reply = tools
            .call(
                "board_open",
                &json!({ "file": "zad/nowa.md", "title": "Szereg", "task": "Zbadaj $\\sum 1/n$." }),
            )
            .unwrap();
        assert!(reply.contains("# Szereg"));
        assert_eq!(current(&tools), dir.path().join("zad/nowa.md"));
    }

    #[test]
    fn open_stays_inside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tools(dir.path());
        for file in ["../x.md", "/tmp/x.md", "notes.txt"] {
            let args = json!({ "file": file, "title": "t", "task": "t" });
            assert!(tools.call("board_open", &args).is_err(), "{file}");
        }
    }
}
