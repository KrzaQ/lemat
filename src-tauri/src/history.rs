//! The conversation's history, which Lemat owns: one JSON object per line,
//! in `lemat.jsonl` in the board's folder. All boards in one folder share
//! one conversation, so the agent keeps its context when it opens another
//! board there.
//!
//! The engine's session only saves sending the history again. A `session`
//! line records each engine session, so that the next start resumes it.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::files::Files;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Entry {
    /// From here on, the engine `engine` knows the conversation as session
    /// `id`.
    Session {
        engine: String,
        id: String,
        time: i64,
    },
    User {
        text: String,
        time: i64,
    },
    /// `work` is the agent's thinking, tool calls and tokens
    /// (`lejce::Activity::to_json`).
    Agent {
        text: String,
        work: Value,
        time: i64,
        /// Who answered: "claude · opus". Missing in older histories.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
    },
    /// A message that ended without an answer: an error, or the user
    /// stopped it.
    Error {
        text: String,
        work: Value,
        time: i64,
    },
}

pub const FILE: &str = "lemat.jsonl";

pub fn path_for(board: &Path) -> PathBuf {
    board.with_file_name(FILE)
}

/// A missing file is an empty history. A broken line is skipped: one bad
/// line must not lose the rest.
pub fn load(files: &dyn Files, path: &Path) -> Vec<Entry> {
    let Ok(text) = files.read(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

pub fn append(files: &dyn Files, path: &Path, entry: &Entry) -> io::Result<()> {
    let mut line = serde_json::to_string(entry).map_err(io::Error::other)?;
    line.push('\n');
    files.append(path, &line)
}

/// The session to resume: the latest one, if the same engine made it. After
/// a change of engine the new engine starts from the history.
pub fn last_session(entries: &[Entry], engine: &str) -> Option<String> {
    entries.iter().rev().find_map(|entry| match entry {
        Entry::Session { engine: e, id, .. } => Some((e == engine).then(|| id.clone())),
        _ => None,
    })?
}

/// The first message of a new engine session: the conversation so far,
/// then the new message. Without earlier messages, the message alone.
pub fn replay(entries: &[Entry], message: &str) -> String {
    let mut earlier = String::new();
    for entry in entries {
        match entry {
            Entry::User { text, .. } => earlier.push_str(&format!("<user>\n{text}\n</user>\n")),
            Entry::Agent { text, .. } => earlier.push_str(&format!("<agent>\n{text}\n</agent>\n")),
            _ => {}
        }
    }
    if earlier.is_empty() {
        return message.to_string();
    }
    format!(
        "This conversation continues an earlier one, whose engine session is gone. \
         The earlier messages:\n\n<history>\n{earlier}</history>\n\n\
         The board holds the current state: read it with board_read. Answer in the \
         language of the user's messages. The new message:\n\n{message}"
    )
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::Local;
    use serde_json::json;
    use std::fs;
    use std::io::Write;

    fn user(text: &str) -> Entry {
        Entry::User {
            text: text.into(),
            time: 1,
        }
    }

    fn agent(text: &str) -> Entry {
        Entry::Agent {
            text: text.into(),
            work: json!({}),
            time: 2,
            by: None,
        }
    }

    fn session(engine: &str, id: &str) -> Entry {
        Entry::Session {
            engine: engine.into(),
            id: id.into(),
            time: 0,
        }
    }

    #[test]
    fn history_sits_next_to_the_board() {
        assert_eq!(
            path_for(Path::new("/n/granica.md")),
            PathBuf::from("/n/lemat.jsonl")
        );
    }

    #[test]
    fn append_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.lemat.jsonl");
        let entries = vec![session("claude", "s1"), user("dziel przez n"), agent("ok")];
        for entry in &entries {
            append(&Local, &path, entry).unwrap();
        }
        assert_eq!(load(&Local, &path), entries);
    }

    #[test]
    fn broken_line_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.lemat.jsonl");
        append(&Local, &path, &user("a")).unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{ broken\n")
            .unwrap();
        append(&Local, &path, &user("b")).unwrap();
        assert_eq!(load(&Local, &path), vec![user("a"), user("b")]);
    }

    #[test]
    fn resumes_only_the_same_engine() {
        let entries = vec![session("claude", "s1"), user("a"), session("pi", "s2")];
        assert_eq!(last_session(&entries, "pi").as_deref(), Some("s2"));
        // The latest session is pi's; Claude's older one lacks the pi turns.
        assert_eq!(last_session(&entries, "claude"), None);
        assert_eq!(last_session(&[], "claude"), None);
    }

    #[test]
    fn replay_carries_earlier_messages() {
        assert_eq!(replay(&[session("claude", "s")], "hej"), "hej");
        let text = replay(&[user("a"), agent("b")], "c");
        assert!(text.contains("<user>\na\n</user>\n<agent>\nb\n</agent>"));
        assert!(text.ends_with("c"));
    }
}
