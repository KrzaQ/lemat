//! The conversation with the agent, through Lejce.
//!
//! One worker thread owns the engine process and handles one message at a
//! time. The web view sends messages with `chat_send` and gets back events:
//! `chat` (a new history entry), `chat-progress` (the agent's work while it
//! runs) and `chat-running`.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use lejce::{
    Answer, Canceller, Conversation, Engine, Error, Host, McpServer, Permissions, Question,
    Session, Setup, Tools,
};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::history::{self, Entry};
use crate::project::{Place, Project, Target};
use crate::settings::Settings;

pub struct Chat {
    jobs: Mutex<Sender<Job>>,
    /// Set while a message runs, so that Stop can reach its engine.
    running: Mutex<Option<Canceller>>,
    /// Set while the agent waits for the user's answer to a question.
    pending: Mutex<Option<Sender<Answer>>>,
}

struct Job {
    place: Place,
    text: String,
}

/// What the web view needs to draw the conversation of a board.
#[derive(Serialize)]
pub struct State {
    pub entries: Vec<Entry>,
    pub running: bool,
    /// The engine and model the next message goes to.
    pub engine: String,
    pub model: String,
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
            pending: Mutex::new(None),
        }
    }

    pub fn state(&self, place: &Place) -> State {
        let settings = settings_now(&place.project, &place.board)
            .unwrap_or_else(|_| place.project.settings.clone());
        State {
            entries: history::load(place.project.files.as_ref(), &place.history()),
            running: self.running.lock().unwrap().is_some(),
            engine: settings.engine,
            model: settings.model,
        }
    }

    pub fn send(&self, place: Place, text: String) {
        let _ = self.jobs.lock().unwrap().send(Job { place, text });
    }

    /// Stops the running message, with the tools the engine started. A
    /// question that waits for the user ends unanswered.
    pub fn stop(&self) {
        if let Some(canceller) = *self.running.lock().unwrap() {
            canceller.cancel();
        }
        self.answer(Answer::Unanswered);
    }

    /// The user's answer to the question the agent waits on.
    pub fn answer(&self, answer: Answer) {
        if let Some(waiting) = self.pending.lock().unwrap().take() {
            let _ = waiting.send(answer);
        }
    }
}

/// A question as the web view shows it.
#[derive(Serialize, Clone)]
struct QuestionView<'a> {
    tool: Option<&'a str>,
    text: &'a str,
    detail: &'a str,
    options: &'a [String],
    multiple: bool,
}

/// The answer in words, for the history.
fn said(answer: &Answer, question: &Question) -> String {
    match answer {
        Answer::Yes => "tak".into(),
        Answer::No => "nie".into(),
        Answer::Unanswered => "bez odpowiedzi".into(),
        Answer::Chose(chosen) => chosen
            .iter()
            .filter_map(|&i| question.options.get(i).cloned())
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Asks the user in a dialog and waits. The window comes to the front, so
/// that the keys of a game pad (Return, Escape, arrows) reach the dialog.
fn ask_user(app: &AppHandle, question: &Question, record: &dyn Fn(Entry)) -> Answer {
    // The board's own tools only write steps and marks. A dialog for each
    // step would bury the proof in questions.
    if question
        .tool
        .as_deref()
        .is_some_and(|t| t.starts_with("mcp__lemat__"))
    {
        return Answer::Yes;
    }
    let (answer_to, answers) = mpsc::channel();
    *app.state::<Chat>().pending.lock().unwrap() = Some(answer_to);
    let view = QuestionView {
        tool: question.tool.as_deref(),
        text: &question.text,
        detail: &question.detail,
        options: &question.options,
        multiple: question.multiple,
    };
    let _ = app.emit("chat-question", view);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let answer = answers.recv().unwrap_or(Answer::Unanswered);
    app.state::<Chat>().pending.lock().unwrap().take();
    let _ = app.emit("chat-question", Value::Null);
    let text = match &question.tool {
        Some(tool) => format!("{tool}: {}", question.text),
        None => question.text.clone(),
    };
    record(Entry::Question {
        text,
        detail: question.detail.clone(),
        answer: said(&answer, question),
        time: history::now(),
    });
    answer
}

/// The open engine and what it was opened for. Another project, a board
/// with another history, or different settings need a new engine process.
struct Open {
    project: Arc<Project>,
    history: PathBuf,
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

/// The settings as they are now, so that a change of model or engine
/// applies to the next message. Where the files live stays as it was when
/// the project opened: that needs a new connection, so it waits for the
/// next open.
pub fn settings_now(project: &Project, board: &std::path::Path) -> Result<Settings, String> {
    match &project.source {
        Target::Board(_) => Settings::for_board(board),
        Target::Project(file) => {
            let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
            let mut settings = Settings::parse(&text)?;
            settings.host.clone_from(&project.settings.host);
            settings.folder.clone_from(&project.settings.folder);
            settings.board.clone_from(&project.settings.board);
            Ok(settings)
        }
    }
}

/// Model names to offer for an engine. Claude Code takes its aliases; pi
/// lists what its providers have, on the machine where it runs.
pub fn models(engine: &str, host: &Host) -> Vec<String> {
    match Engine::parse(engine) {
        Some(Engine::Claude) => ["opus", "sonnet", "haiku"].map(String::from).to_vec(),
        Some(Engine::Pi) => host
            .command("pi", &["--list-models".to_string()])
            .output()
            .map(|out| pi_models(&String::from_utf8_lossy(&out.stdout)))
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// `pi --list-models` prints a table: provider, model, and more columns.
fn pi_models(table: &str) -> Vec<String> {
    table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            Some(format!("{}/{}", words.next()?, words.next()?))
        })
        .collect()
}

/// Where the engine of this place runs, with the settings as they are now.
pub fn engine_host(place: &Place, settings: &Settings) -> Host {
    match &place.project.source {
        Target::Board(_) => settings.host(&place.board),
        Target::Project(_) => place.project.engine.clone(),
    }
}

/// Lejce's failures, said so that the user knows what to do.
fn explain(error: &Error, host: Option<&str>) -> String {
    let text = error.to_string();
    let server = host.unwrap_or("serwerem");
    if text.contains("port forwarding failed") {
        return format!(
            "Port tunelu do narzędzi tablicy na {server} jest zajęty. Wyślij jeszcze raz."
        );
    }
    const SSH: [&str; 6] = [
        "Could not resolve hostname",
        "Connection refused",
        "timed out",
        "Permission denied",
        "Host key verification failed",
        "Connection closed",
    ];
    if let Some(line) = text.lines().find(|l| SSH.iter().any(|s| l.contains(s))) {
        return format!(
            "Nie można połączyć się z {server} przez SSH: {}",
            line.trim()
        );
    }
    if text.contains("No such file or directory") && text.contains("cd") {
        return format!("Na {server} nie ma folderu projektu.");
    }
    if text.contains("command not found") {
        return format!("Na {server} nie ma programu silnika: {}", text.trim());
    }
    match error {
        Error::Cancelled => "Zatrzymano.".into(),
        _ => text,
    }
}

fn handle(app: &AppHandle, open: &mut Option<Open>, job: Job, board_tools: &str) {
    let place = &job.place;
    let files = place.project.files.clone();
    let path = place.history();
    let earlier = history::load(files.as_ref(), &path);
    let record = |entry: Entry| {
        if let Err(e) = history::append(files.as_ref(), &path, &entry) {
            eprintln!("lemat: cannot save {}: {e}", files.label(&path));
        }
        let _ = app.emit("chat", entry);
    };
    record(Entry::User {
        text: job.text.clone(),
        time: history::now(),
    });

    let settings = match settings_now(&place.project, &place.board) {
        Ok(s) => s,
        Err(e) => return record(failure(format!("Ustawienia: {e}"), json!({}))),
    };
    let same = |o: &Open| {
        Arc::ptr_eq(&o.project, &place.project) && o.history == path && o.settings == settings
    };
    if open.as_ref().is_some_and(|o| !same(o)) {
        if let Some(o) = open.take() {
            o.conversation.close();
        }
    }
    let engine_host = engine_host(place, &settings);
    let host = match &engine_host {
        Host::Ssh { destination, .. } => Some(destination.as_str()),
        Host::Local { .. } => None,
    };

    // A session the engine has lost gets the history in a new session,
    // once. A taken tunnel port gets one more try on another random port.
    let mut session_gone = false;
    let mut port_taken = false;
    loop {
        let mut replay = false;
        if open.is_none() {
            let resume = if session_gone {
                None
            } else {
                history::last_session(&earlier, &settings.engine)
            };
            replay = resume.is_none();
            match start(&engine_host, &settings, resume, board_tools) {
                Ok((conversation, new_session)) => {
                    if let Some(entry) = new_session {
                        record(entry);
                    }
                    *open = Some(Open {
                        project: place.project.clone(),
                        history: path.clone(),
                        settings: settings.clone(),
                        conversation,
                    });
                }
                Err(e) => return record(failure(explain(&e, host), json!({}))),
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
            &|question| ask_user(app, question, &record),
        );
        *app.state::<Chat>().running.lock().unwrap() = None;

        let work = outcome.activity.to_json();
        let error = match outcome.answer {
            Ok(text) => {
                return record(Entry::Agent {
                    text,
                    work,
                    time: history::now(),
                    by: Some(format!("{} · {}", settings.engine, settings.model)),
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
            Error::SessionGone if !session_gone => session_gone = true,
            ref e if !port_taken && e.to_string().contains("port forwarding failed") => {
                port_taken = true
            }
            other => return record(failure(explain(&other, host), work)),
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
    host: &Host,
    settings: &Settings,
    resume: Option<String>,
    board_tools: &str,
) -> Result<(Conversation, Option<Entry>), Error> {
    let engine = settings.engine().map_err(Error::Setup)?;
    let permissions = settings.permissions().map_err(Error::Setup)?;
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
    let setup = Setup {
        engine,
        system: SYSTEM.into(),
        host: host.clone(),
        model: settings.model.clone(),
        effort: settings.effort.clone(),
        disable_hooks: !settings.hooks,
        project: true,
        tools: Some(Tools {
            builtin: builtin_tools(engine, permissions),
            // Forwarded, so that an engine on another machine reaches it too.
            servers: vec![McpServer::http("lemat", board_tools)],
            permissions,
        }),
        session,
        files: std::env::temp_dir().join("lemat-lejce"),
    };
    Ok((Conversation::open(&setup)?, entry))
}

/// The engine's own tools: reading, for the user's notes and course files,
/// and for Claude a shell when the user approves each command, to check a
/// computation. The board changes through the board tools, so that the
/// steps and marks keep their format. pi gets no shell: its questions come
/// only from extensions the user may not have.
fn builtin_tools(engine: Engine, permissions: Permissions) -> Vec<String> {
    let names: &[&str] = match (engine, permissions) {
        (Engine::Claude, Permissions::Ask) => &["Read", "Glob", "Grep", "Bash"],
        (Engine::Claude, Permissions::Auto) => &["Read", "Glob", "Grep"],
        (Engine::Pi, _) => &["read", "grep", "find", "ls"],
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

If you have a shell, you may run a short computation to check a step, \
for example with python; the user approves each command first.

Read the board with board_read at the start of a conversation and whenever the \
user may have edited it. A new task goes on a new board with board_open.

Answer in the chat in one or two sentences, in the language of the user's own \
messages, even when the text around them is in English. Questions \
about the material get an answer in the chat, not on the board.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_read_as_words() {
        let question = Question {
            options: vec!["a".into(), "b".into(), "c".into()],
            ..Question::default()
        };
        assert_eq!(said(&Answer::Yes, &question), "tak");
        assert_eq!(said(&Answer::Chose(vec![0, 2]), &question), "a, c");
        assert_eq!(said(&Answer::Unanswered, &question), "bez odpowiedzi");
    }

    #[test]
    fn shell_only_when_the_user_approves() {
        assert!(builtin_tools(Engine::Claude, Permissions::Ask).contains(&"Bash".to_string()));
        assert!(!builtin_tools(Engine::Claude, Permissions::Auto).contains(&"Bash".to_string()));
        assert!(!builtin_tools(Engine::Pi, Permissions::Ask)
            .iter()
            .any(|t| t == "bash"));
    }

    #[test]
    fn reads_pi_model_table() {
        let table = "provider  model  context\nlocal  qwen-96k  98K\nother  big  1M\n";
        assert_eq!(pi_models(table), ["local/qwen-96k", "other/big"]);
        assert_eq!(models("claude", &Host::Local { folder: None })[0], "opus");
    }

    #[test]
    fn explains_ssh_failures_in_polish() {
        let failed = |stderr: &str| Error::NoAnswer {
            engine: Engine::Claude,
            stderr: stderr.into(),
        };
        let port = failed("Error: remote port forwarding failed for listen port 31337");
        assert!(explain(&port, Some("server")).contains("Port tunelu"));
        let dns = failed("ssh: Could not resolve hostname server: nodename nor servname");
        assert!(explain(&dns, Some("server")).starts_with("Nie można połączyć się z server"));
        let folder = failed("sh: line 1: cd: /x: No such file or directory");
        assert_eq!(
            explain(&folder, Some("server")),
            "Na server nie ma folderu projektu."
        );
        assert_eq!(explain(&Error::Cancelled, None), "Zatrzymano.");
        assert_eq!(
            explain(&failed("boom"), None),
            "claude gave no answer: boom"
        );
    }

    #[test]
    fn prompt_names_every_board_tool() {
        for tool in ["board_read", "board_step", "board_mark", "board_open"] {
            assert!(SYSTEM.contains(tool), "{tool}");
        }
    }
}
