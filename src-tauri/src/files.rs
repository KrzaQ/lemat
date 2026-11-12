//! Where the board files and the history live: on this machine, or in a
//! folder on another machine over SSH. Everything that reads or writes
//! them goes through `Files`.
//!
//! Over SSH each call is one command through a shared connection
//! (ControlMaster), so that polling the board twice a second costs one
//! round trip, not a new login.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

/// What changes when a file changes. Two equal stamps mean the watcher does
/// not need to read the file again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp(String);

impl From<String> for Stamp {
    fn from(text: String) -> Self {
        Stamp(text)
    }
}

pub trait Files: Send + Sync + 'static {
    /// The SSH destination, or None for this machine.
    fn host(&self) -> Option<&str>;
    fn stamp(&self, path: &Path) -> io::Result<Stamp>;
    fn read(&self, path: &Path) -> io::Result<String>;
    /// Replaces the file, and makes its folder if needed.
    fn write(&self, path: &Path, text: &str) -> io::Result<()>;
    /// Adds to the end of the file, and makes the file and its folder if
    /// needed.
    fn append(&self, path: &Path, text: &str) -> io::Result<()>;
    fn exists(&self, path: &Path) -> io::Result<bool>;
    /// How often to look for a change.
    fn poll(&self) -> Duration;
    /// The path as the user reads it: "server:/notes/board.md".
    fn label(&self, path: &Path) -> String {
        match self.host() {
            Some(host) => format!("{host}:{}", path.display()),
            None => path.display().to_string(),
        }
    }
}

pub struct Local;

impl Files for Local {
    fn host(&self) -> Option<&str> {
        None
    }

    fn stamp(&self, path: &Path) -> io::Result<Stamp> {
        let meta = fs::metadata(path)?;
        if meta.is_dir() {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, "folder"));
        }
        let modified = meta.modified().ok().and_then(|m| {
            m.duration_since(SystemTime::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_nanos())
        });
        Ok(Stamp(format!("{modified:?} {}", meta.len())))
    }

    fn read(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, text)
    }

    fn append(&self, path: &Path, text: &str) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(text.as_bytes())
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        path.try_exists()
    }

    fn poll(&self) -> Duration {
        Duration::from_millis(250)
    }
}

/// A shell word: single quotes, with each `'` closed, escaped and reopened.
pub fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// A folder as the remote shell must see it: `~/x` stays under the home
/// folder, the rest is quoted.
fn remote_folder(folder: &str) -> String {
    match folder {
        "~" => "\"$HOME\"".into(),
        _ => match folder.strip_prefix("~/") {
            Some(rest) => format!("\"$HOME\"/{}", quote(rest)),
            None => quote(folder),
        },
    }
}

/// The remote scripts end with these codes, so that Lejce-side errors map
/// to the same kinds as local ones.
const MISSING: i32 = 3;
const FOLDER: i32 = 4;
/// ssh's own failure: it could not connect or log in.
const SSH_FAILED: i32 = 255;

pub struct Ssh {
    destination: String,
}

impl Ssh {
    /// Connects and returns the folder as an absolute path on that machine.
    /// Fails when ssh cannot connect or the folder does not exist.
    pub fn connect(destination: &str, folder: &str) -> io::Result<(Ssh, PathBuf)> {
        let ssh = Ssh {
            destination: destination.to_string(),
        };
        let script = format!(
            "cd {} 2>/dev/null || exit {MISSING}; pwd",
            remote_folder(folder)
        );
        let out = ssh.run(&script, None).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(
                io::ErrorKind::NotFound,
                format!("na {destination} nie ma folderu {folder}"),
            ),
            _ => e,
        })?;
        Ok((ssh, PathBuf::from(out.trim_end_matches('\n'))))
    }

    fn command(&self) -> Command {
        let mut command = Command::new("ssh");
        command.args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ControlMaster=auto",
            // Short and fixed: a Unix socket path has a length limit.
            "-o",
            "ControlPath=/tmp/lemat-ssh-%C",
            "-o",
            "ControlPersist=600",
            "-T",
            &self.destination,
        ]);
        command
    }

    /// Runs a script there, with `input` on its standard input.
    fn run(&self, script: &str, input: Option<&str>) -> io::Result<String> {
        let mut child = self
            .command()
            .arg(script)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(text.as_bytes())?;
        }
        let out = child.wait_with_output()?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("");
        match out.status.code() {
            Some(0) => String::from_utf8(out.stdout)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8")),
            Some(MISSING) => Err(io::Error::new(io::ErrorKind::NotFound, "missing")),
            Some(FOLDER) => Err(io::Error::new(io::ErrorKind::IsADirectory, "folder")),
            // The caller names the host; ssh's own line says what failed.
            Some(SSH_FAILED) => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                last.to_string(),
            )),
            _ => Err(io::Error::other(last.to_string())),
        }
    }
}

fn file(path: &Path) -> String {
    format!("p={}; ", quote(&path.to_string_lossy()))
}

// The scripts that run on the other machine, one per call.

fn stamp_script(path: &Path) -> String {
    // GNU stat first, BSD stat for a Mac on the other side.
    format!(
        "{}[ -e \"$p\" ] || exit {MISSING}; [ -d \"$p\" ] && exit {FOLDER}; \
         stat -c '%y %s' -- \"$p\" 2>/dev/null || stat -f '%Fm %z' -- \"$p\"",
        file(path)
    )
}

fn read_script(path: &Path) -> String {
    format!(
        "{}[ -e \"$p\" ] || exit {MISSING}; [ -d \"$p\" ] && exit {FOLDER}; cat -- \"$p\"",
        file(path)
    )
}

fn write_script(path: &Path) -> String {
    // A new file and a rename: a reader never sees half a board.
    format!(
        "{}mkdir -p -- \"$(dirname -- \"$p\")\" && cat > \"$p.lemat-new\" && \
         mv -f -- \"$p.lemat-new\" \"$p\"",
        file(path)
    )
}

fn append_script(path: &Path) -> String {
    format!(
        "{}mkdir -p -- \"$(dirname -- \"$p\")\" && cat >> \"$p\"",
        file(path)
    )
}

fn exists_script(path: &Path) -> String {
    format!("{}[ -e \"$p\" ] || exit {MISSING}", file(path))
}

impl Files for Ssh {
    fn host(&self) -> Option<&str> {
        Some(&self.destination)
    }

    fn stamp(&self, path: &Path) -> io::Result<Stamp> {
        self.run(&stamp_script(path), None)
            .map(|s| Stamp(s.trim().to_string()))
    }

    fn read(&self, path: &Path) -> io::Result<String> {
        self.run(&read_script(path), None)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        self.run(&write_script(path), Some(text)).map(|_| ())
    }

    fn append(&self, path: &Path, text: &str) -> io::Result<()> {
        self.run(&append_script(path), Some(text)).map(|_| ())
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        match self.run(&exists_script(path), None) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn poll(&self) -> Duration {
        Duration::from_millis(500)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_changes_with_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.md");
        Local.write(&path, "a").unwrap();
        let first = Local.stamp(&path).unwrap();
        Local.write(&path, "ab").unwrap();
        assert_ne!(Local.stamp(&path).unwrap(), first);
        assert_eq!(Local.read(&path).unwrap(), "ab");
    }

    #[test]
    fn folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = Local.stamp(dir.path()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::IsADirectory);
    }

    #[test]
    fn write_and_append_make_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/history.jsonl");
        Local.append(&path, "1\n").unwrap();
        Local.append(&path, "2\n").unwrap();
        assert_eq!(Local.read(&path).unwrap(), "1\n2\n");
        let board = dir.path().join("c/board.md");
        assert!(!Local.exists(&board).unwrap());
        Local.write(&board, "x").unwrap();
        assert!(Local.exists(&board).unwrap());
    }

    #[test]
    fn label_names_the_host() {
        assert_eq!(Local.label(Path::new("/n/b.md")), "/n/b.md");
        let ssh = Ssh {
            destination: "server".into(),
        };
        assert_eq!(ssh.label(Path::new("/n/b.md")), "server:/n/b.md");
    }

    #[test]
    fn quoting_keeps_the_shell_out() {
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(remote_folder("~"), "\"$HOME\"");
        assert_eq!(remote_folder("~/my notes"), "\"$HOME\"/'my notes'");
        assert_eq!(remote_folder("/srv/x"), "'/srv/x'");
    }

    /// The SSH side runs the same scripts through `sh -c` on this machine,
    /// so that the tests need no server.
    fn run_here(script: &str, input: Option<&str>) -> (i32, String) {
        let mut child = Command::new("sh")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(input.unwrap_or("").as_bytes()).unwrap();
        drop(stdin);
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
        )
    }

    #[test]
    fn remote_scripts_work_in_a_shell() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub dir/it's.md");
        assert_eq!(run_here(&exists_script(&path), None).0, MISSING);
        assert_eq!(run_here(&read_script(&path), None).0, MISSING);
        assert_eq!(run_here(&write_script(&path), Some("# T\n")).0, 0);
        assert_eq!(fs::read_to_string(&path).unwrap(), "# T\n");
        assert_eq!(run_here(&append_script(&path), Some("more\n")).0, 0);
        assert_eq!(
            run_here(&read_script(&path), None),
            (0, "# T\nmore\n".into())
        );
        assert_eq!(run_here(&exists_script(&path), None).0, 0);
        let (code, stamp) = run_here(&stamp_script(&path), None);
        assert_eq!(code, 0);
        assert!(stamp.trim().ends_with(" 9"), "{stamp}");
        assert_eq!(run_here(&stamp_script(dir.path()), None).0, FOLDER);
    }
}
