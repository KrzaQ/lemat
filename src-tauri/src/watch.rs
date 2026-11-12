//! Polls a board source and reports each change once.
//!
//! Polling, not file system events: it works the same for a remote source,
//! and it survives editors that save by writing a new file and renaming it.

use std::io;

use crate::source::{BoardSource, Stamp};

#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Stamp(Stamp),
    Error(String),
}

pub struct Watch<S> {
    source: S,
    last: Option<Seen>,
}

impl<S: BoardSource> Watch<S> {
    pub fn new(source: S) -> Self {
        Self { source, last: None }
    }

    #[cfg(test)]
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Returns the new text or the new error, or `None` when nothing changed
    /// since the last call. The first call always returns something.
    pub fn poll(&mut self) -> Option<Result<String, String>> {
        let result = self
            .source
            .stamp()
            .and_then(|stamp| self.source.read().map(|text| (stamp, text)));
        let (seen, out) = match result {
            Ok((stamp, text)) => {
                if self.last == Some(Seen::Stamp(stamp)) {
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
        _ => format!("Nie można odczytać pliku: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{Duration, SystemTime};

    /// A source whose next answer the test sets.
    struct Fake(Mutex<io::Result<(u64, String)>>);

    impl Fake {
        fn new(len: u64, text: &str) -> Self {
            Self(Mutex::new(Ok((len, text.into()))))
        }
        fn set(&self, next: io::Result<(u64, String)>) {
            *self.0.lock().unwrap() = next;
        }
    }

    impl BoardSource for Fake {
        fn name(&self) -> String {
            "fake".into()
        }
        fn location(&self) -> String {
            "fake".into()
        }
        fn stamp(&self) -> io::Result<Stamp> {
            match &*self.0.lock().unwrap() {
                Ok((len, _)) => Ok(Stamp {
                    modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(*len)),
                    len: *len,
                }),
                Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
            }
        }
        fn read(&self) -> io::Result<String> {
            match &*self.0.lock().unwrap() {
                Ok((_, text)) => Ok(text.clone()),
                Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
            }
        }
    }

    fn missing() -> io::Result<(u64, String)> {
        Err(io::Error::new(io::ErrorKind::NotFound, "gone"))
    }

    #[test]
    fn reports_each_change_once() {
        let mut watch = Watch::new(Fake::new(1, "a"));
        assert_eq!(watch.poll(), Some(Ok("a".into())));
        assert_eq!(watch.poll(), None);
        watch.source().set(Ok((2, "b".into())));
        assert_eq!(watch.poll(), Some(Ok("b".into())));
        assert_eq!(watch.poll(), None);
    }

    #[test]
    fn reports_a_missing_file_once_and_its_return() {
        let mut watch = Watch::new(Fake::new(1, "a"));
        watch.poll();
        watch.source().set(missing());
        assert_eq!(watch.poll(), Some(Err("Nie ma tego pliku.".into())));
        assert_eq!(watch.poll(), None);
        // The same text as before the file went away must still come back:
        // the board shows the note until it gets text again.
        watch.source().set(Ok((1, "a".into())));
        assert_eq!(watch.poll(), Some(Ok("a".into())));
    }

    #[test]
    fn first_poll_reports_an_error() {
        let fake = Fake::new(0, "");
        fake.set(missing());
        let mut watch = Watch::new(fake);
        assert!(matches!(watch.poll(), Some(Err(_))));
    }

    #[test]
    fn sees_a_real_file_change() {
        use crate::source::LocalFile;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.md");
        std::fs::write(&path, "# one").unwrap();
        let mut watch = Watch::new(LocalFile::new(path.clone()));
        assert_eq!(watch.poll(), Some(Ok("# one".into())));
        std::fs::write(&path, "# two, longer").unwrap();
        assert_eq!(watch.poll(), Some(Ok("# two, longer".into())));
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(watch.poll(), Some(Err(_))));
    }
}
