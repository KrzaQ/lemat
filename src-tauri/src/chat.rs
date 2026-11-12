//! The conversation with the agent, through Lejce.
//!
//! One worker thread owns the engine process and handles one message at a
//! time. The web view sends messages with `chat_send` and gets back events:
//! `chat` (a new history entry), `chat-progress` (the agent's work while it
//! runs) and `chat-running`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;

use lejce::{Answer, Canceller, Conversation, Engine, Error, McpServer, Session, Setup, Tools};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::history::{self, Entry};
use crate::settings::Settings;

pub struct Chat {
    jobs: Mutex<Sender<Job>>,
    /// Set while a message runs, so that Stop can reach its engine.
    running: Mutex<Option<Canceller>>,
}

struct Job {
    board: PathBuf,
    text: String,
}

/// What the web view needs to draw the conversation of a board.
#[derive(Serialize)]
pub struct State {
    pub entries: Vec<Entry>,
    pub running: bool,
}

impl Chat {
    /// `board_tools` is the URL of the board's MCP server.
    pub fn start(app: &AppHandle, board_tools: String) -> Self {
        let (jobs, inbox) = mpsc::channel();
        let app = app.clone();
        std::thread::spawn(move || work(app, inbox, board_tools));
        Self {
            jobs: Mutex::new(jobs),
            running: Mutex::new(None),
        }
    }

    pub fn state(&self, board: &Path) -> State {
        State {
            entries: history::load(&history::path_for(board)),
            running: self.running.lock().unwrap().is_some(),
        }
    }

    pub fn send(&self, board: PathBuf, text: String) {
        let _ = self.jobs.lock().unwrap().send(Job { board, text });
    }

    /// Stops the running message, with the tools the engine started.
    pub fn stop(&self) {
        if let Some(canceller) = *self.running.lock().unwrap() {
            canceller.cancel();
        }
    }
}

/// The open engine and what it was opened for. A board in another folder
/// (another project) or different settings need a new engine process.
struct Open {
    folder: PathBuf,
    settings: Settings,
    conversation: Conversation,
}

fn work(app: AppHandle, inbox: Receiver<Job>, board_tools: String) {
    let mut open: Option<Open> = None;
    for job in inbox {
        let _ = app.emit("chat-running", true);
        handle(&app, &mut open, job, &board_tools);
        let _ = app.emit("chat-running", false);
    }
}

fn handle(app: &AppHandle, open: &mut Option<Open>, job: Job, board_tools: &str) {
    let path = history::path_for(&job.board);
    let folder = job
        .board
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let earlier = history::load(&path);
    let record = |entry: Entry| {
        if let Err(e) = history::append(&path, &entry) {
            eprintln!("lemat: cannot save {}: {e}", path.display());
        }
        let _ = app.emit("chat", entry);
    };
    record(Entry::User {
        text: job.text.clone(),
        time: history::now(),
    });

    let settings = match Settings::for_board(&job.board) {
        Ok(s) => s,
        Err(e) => return record(failure(format!("Ustawienia: {e}"), json!({}))),
    };
    if open
        .as_ref()
        .is_some_and(|o| o.folder != folder || o.settings != settings)
    {
        if let Some(o) = open.take() {
            o.conversation.close();
        }
    }

    // A new engine session first gets the history; one that the engine
    // has lost gets it in a new session, once.
    for attempt in 0..2 {
        let mut replay = false;
        if open.is_none() {
            let resume = if attempt == 0 {
                history::last_session(&earlier, &settings.engine)
            } else {
                None
            };
            replay = resume.is_none();
            match start(&job.board, &settings, resume, board_tools) {
                Ok((conversation, new_session)) => {
                    if let Some(entry) = new_session {
                        record(entry);
                    }
                    *open = Some(Open {
                        folder: folder.clone(),
                        settings: settings.clone(),
                        conversation,
                    });
                }
                Err(e) => return record(failure(e.to_string(), json!({}))),
            }
        }
        let current = open.as_mut().expect("opened above");
        let message = if replay {
            history::replay(&earlier, &job.text)
        } else {
            job.text.clone()
        };

        *app.state::<Chat>().running.lock().unwrap() = Some(current.conversation.canceller());
        let outcome = current.conversation.send(
            &message,
            &|activity| {
                let _ = app.emit("chat-progress", activity.to_json());
            },
            // Questions in a dialog are a later step of the design.
            &|_| Answer::Unanswered,
        );
        *app.state::<Chat>().running.lock().unwrap() = None;

        let work = outcome.activity.to_json();
        let error = match outcome.answer {
            Ok(text) => {
                return record(Entry::Agent {
                    text,
                    work,
                    time: history::now(),
                })
            }
            Err(error) => error,
        };
        // After an error the engine may have quit. The next message resumes
        // the session in a new process.
        if let Some(o) = open.take() {
            o.conversation.close();
        }
        match error {
            Error::SessionGone if attempt == 0 => continue,
            Error::Cancelled => return record(failure("Zatrzymano.".into(), work)),
            other => return record(failure(other.to_string(), work)),
        }
    }
}

fn failure(text: String, work: Value) -> Entry {
    Entry::Error {
        text,
        work,
        time: history::now(),
    }
}

/// Starts the engine. With no session to resume, it starts a new one and
/// returns the history entry that names it.
fn start(
    board: &Path,
    settings: &Settings,
    resume: Option<String>,
    board_tools: &str,
) -> Result<(Conversation, Option<Entry>), Error> {
    let engine = settings.engine().map_err(Error::Setup)?;
    let (session, entry) = match resume {
        Some(id) => (Session::Resume(id), None),
        None => {
            let id = lejce::new_session_id();
            let entry = Entry::Session {
                engine: settings.engine.clone(),
                id: id.clone(),
                time: history::now(),
            };
            (Session::New(id), Some(entry))
        }
    };
    let host = settings.host(board);
    let setup = Setup {
        engine,
        system: SYSTEM.into(),
        host,
        model: settings.model.clone(),
        effort: settings.effort.clone(),
        project: true,
        tools: Some(Tools {
            builtin: reading_tools(engine),
            // Forwarded, so that an engine on another machine reaches it too.
            servers: vec![McpServer::http("lemat", board_tools)],
        }),
        session,
        files: std::env::temp_dir().join("lemat-lejce"),
    };
    Ok((Conversation::open(&setup)?, entry))
}

/// The engine's own tools: only reading, for the user's notes and course
/// files. The board changes through the board tools, so that the steps and
/// marks keep their format.
fn reading_tools(engine: Engine) -> Vec<String> {
    let names: &[&str] = match engine {
        Engine::Claude => &["Read", "Glob", "Grep"],
        Engine::Pi => &["read", "grep", "find", "ls"],
    };
    names.iter().map(|n| n.to_string()).collect()
}

const SYSTEM: &str = "\
You work with the user in Lemat, a math board. The board is a Markdown document \
with LaTeX that the user sees next to this conversation. Change it only through \
the board tools (board_read, board_step, board_mark, board_open), never by \
editing its file.

The user leads a proof or a calculation, one sentence at a time, often by voice. \
For each sentence:
1. Write it as the next step with board_step: a short note in words in the \
user's language, and the formula in LaTeX if the step has one. Write what the \
user said, even if it is wrong.
2. Check the step. Then mark it with board_mark: checked if it is right, wrong \
with one sentence that says what is wrong.
3. Do not fix a wrong step and do not take the next step yourself unless the \
user asks: the user leads the proof.

Read the board with board_read at the start of a conversation and whenever the \
user may have edited it. A new task goes on a new board with board_open.

Answer in the chat in one or two sentences, in the user's language. Questions \
about the material get an answer in the chat, not on the board.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_names_every_board_tool() {
        for tool in ["board_read", "board_step", "board_mark", "board_open"] {
            assert!(SYSTEM.contains(tool), "{tool}");
        }
    }
}
