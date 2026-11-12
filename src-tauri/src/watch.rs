//! Polls a board file and reports each change once.
//!
//! Polling, not file system events: it works the same over SSH, and it
//! survives editors that save by writing a new file and renaming it.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::files::{Files, Stamp};

#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Stamp(Stamp),
    Error(String),
}

pub struct Watch {
    files: Arc<dyn Files>,
    path: PathBuf,
    last: Option<Seen>,
}

impl Watch {
    pub fn new(files: Arc<dyn Files>, path: PathBuf) -> Self {
        Self {
            files,
            path,
            last: None,
        }
    }

    /// Returns the new text or the new error, or `None` when nothing changed
    /// since the last call. The first call always returns something.
    pub fn poll(&mut self) -> Option<Result<String, String>> {
        let result = self
            .files
            .stamp(&self.path)
            .and_then(|stamp| self.files.read(&self.path).map(|text| (stamp, text)));
        let (seen, out) = match result {
            Ok((stamp, text)) => {
                if self.last.as_ref() == Some(&Seen::Stamp(stamp.clone())) {
                    return None;
                }
                (Seen::Stamp(stamp), Ok(text))
            }
            Err(e) => {
                let message = describe(&e);
                (Seen::Error(message.clone()), Err(message))
            }
        };
        if self.last.as_ref() == Some(&seen) {
            return None;
        }
        self.last = Some(seen);
        Some(out)
    }
}

/// The note the board shows when it cannot read the file.
pub fn describe(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::NotFound => "Nie ma tego pliku.".into(),
        io::ErrorKind::PermissionDenied => "Brak dostępu do pliku.".into(),
        io::ErrorKind::IsADirectory => "To jest folder, nie plik.".into(),
        io::ErrorKind::InvalidData => "Plik nie jest tekstem UTF-8.".into(),
        io::ErrorKind::NotConnected => format!("Brak połączenia z serwerem ({e})."),
        _ => format!("Nie można odczytać pliku: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;
    use std::time::Duration;

    /// Files whose one file the test sets: its stamp and text, or an error.
    struct Fake(Mutex<io::Result<(u64, String)>>);

    impl Fake {
        fn new(stamp: u64, text: &str) -> Arc<Self> {
            Arc::new(Self(Mutex::new(Ok((stamp, text.into())))))
        }
        fn set(&self, next: io::Result<(u64, String)>) {
            *self.0.lock().unwrap() = next;
        }
        fn get(&self) -> io::Result<(u64, String)> {
            match &*self.0.lock().unwrap() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
            }
        }
    }

    impl Files for Fake {
        fn host(&self) -> Option<&str> {
            None
        }
        fn stamp(&self, _: &Path) -> io::Result<Stamp> {
            self.get().map(|(n, _)| Stamp::from(n.to_string()))
        }
        fn read(&self, _: &Path) -> io::Result<String> {
            self.get().map(|(_, text)| text)
        }
        fn write(&self, _: &Path, _: &str) -> io::Result<()> {
            unimplemented!()
        }
        fn append(&self, _: &Path, _: &str) -> io::Result<()> {
            unimplemented!()
        }
        fn exists(&self, _: &Path) -> io::Result<bool> {
            Ok(self.get().is_ok())
        }
        fn poll(&self) -> Duration {
            Duration::ZERO
        }
    }

    fn watch(fake: &Arc<Fake>) -> Watch {
        Watch::new(fake.clone(), PathBuf::from("board.md"))
    }

    fn missing() -> io::Result<(u64, String)> {
        Err(io::Error::new(io::ErrorKind::NotFound, "gone"))
    }

    #[test]
    fn reports_each_change_once() {
        let fake = Fake::new(1, "a");
        let mut watch = watch(&fake);
        assert_eq!(watch.poll(), Some(Ok("a".into())));
        assert_eq!(watch.poll(), None);
        fake.set(Ok((2, "b".into())));
        assert_eq!(watch.poll(), Some(Ok("b".into())));
        assert_eq!(watch.poll(), None);
    }

    #[test]
    fn reports_a_missing_file_once_and_its_return() {
        let fake = Fake::new(1, "a");
        let mut watch = watch(&fake);
        watch.poll();
        fake.set(missing());
        assert_eq!(watch.poll(), Some(Err("Nie ma tego pliku.".into())));
        assert_eq!(watch.poll(), None);
        // The same text as before the file went away must still come back:
        // the board shows the note until it gets text again.
        fake.set(Ok((1, "a".into())));
        assert_eq!(watch.poll(), Some(Ok("a".into())));
    }

    #[test]
    fn a_lost_connection_has_its_own_note() {
        let fake = Fake::new(1, "a");
        fake.set(Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "ssh server: timeout",
        )));
        let message = watch(&fake).poll().unwrap().unwrap_err();
        assert!(
            message.starts_with("Brak połączenia z serwerem"),
            "{message}"
        );
    }

    #[test]
    fn sees_a_real_file_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.md");
        std::fs::write(&path, "# one").unwrap();
        let mut watch = Watch::new(Arc::new(crate::files::Local), path.clone());
        assert_eq!(watch.poll(), Some(Ok("# one".into())));
        std::fs::write(&path, "# two, longer").unwrap();
        assert_eq!(watch.poll(), Some(Ok("# two, longer".into())));
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(watch.poll(), Some(Err(_))));
    }
}
