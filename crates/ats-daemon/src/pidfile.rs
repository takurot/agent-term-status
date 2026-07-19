//! PID file management with stale detection (I-13).
//!
//! On startup the daemon acquires a PID file. If the file already exists
//! and its PID belongs to a live process, startup is refused. If the PID
//! is dead (e.g. after `kill -9`), the stale file is replaced.

use std::io;
use std::path::{Path, PathBuf};

/// PID file acquisition failure.
#[derive(Debug, thiserror::Error)]
pub enum PidFileError {
    /// Another daemon instance owns the PID file.
    #[error("another ats-daemon is already running (pid {pid})")]
    AlreadyRunning {
        /// PID of the live daemon.
        pid: i32,
    },
    /// Filesystem error while reading or writing the PID file.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Guard for an acquired PID file. Removes the file on drop.
#[derive(Debug)]
pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    /// Acquires the PID file at `path`, replacing stale files whose PID
    /// no longer refers to a live process.
    ///
    /// Uses `O_EXCL` creation so two concurrently starting daemons can
    /// never both own the file: the loser of the create race re-reads
    /// the file and sees the winner's live PID.
    pub fn acquire(path: &Path) -> Result<Self, PidFileError> {
        const MAX_ATTEMPTS: usize = 3;

        for _ in 0..MAX_ATTEMPTS {
            match write_own_pid_exclusive(path) {
                Ok(()) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                    })
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    match std::fs::read_to_string(path) {
                        Ok(contents) => {
                            if let Ok(pid) = contents.trim().parse::<i32>() {
                                if pid != std::process::id() as i32 && process_alive(pid) {
                                    return Err(PidFileError::AlreadyRunning { pid });
                                }
                            }
                            // Unparseable or dead PID: stale file.
                        }
                        // File vanished between create and read: retry.
                        Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                        Err(e) => return Err(e.into()),
                    }
                    match std::fs::remove_file(path) {
                        Ok(()) => {}
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.into()),
                    }
                    // Retry exclusive creation after unlinking the stale file.
                }
                Err(e) => return Err(e.into()),
            }
        }

        Err(PidFileError::Io(io::Error::new(
            io::ErrorKind::WouldBlock,
            "PID file contention: another process keeps recreating it",
        )))
    }

    /// Path of the PID file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn write_own_pid_exclusive(path: &Path) -> io::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    write!(file, "{}", std::process::id())?;
    Ok(())
}

/// Returns `true` when `pid` refers to a live process.
///
/// `kill(pid, 0)` succeeds for live processes; `EPERM` means the process
/// exists but belongs to another user (treated as alive — never clobber).
fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // SAFETY: kill with signal 0 performs error checking only; it sends
    // no signal and cannot affect the target process.
    let ret = unsafe { libc::kill(pid, 0) };
    if ret == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dead_pid() -> i32 {
        // Spawn a child and wait for it; its PID is guaranteed dead and
        // extremely unlikely to be reused within this test.
        let child = std::process::Command::new("true")
            .spawn()
            .expect("spawn `true`");
        let pid = child.id() as i32;
        let mut child = child;
        child.wait().expect("wait for child");
        pid
    }

    #[test]
    fn acquire_creates_pid_file_with_own_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("status.pid");

        let guard = PidFile::acquire(&path).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, std::process::id().to_string());
        drop(guard);
    }

    #[test]
    fn drop_removes_pid_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("status.pid");

        let guard = PidFile::acquire(&path).unwrap();
        assert!(path.exists());
        drop(guard);
        assert!(!path.exists());
    }

    #[test]
    fn live_pid_refuses_acquisition() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("status.pid");
        // PID 1 (launchd/init) is always alive and never ours.
        std::fs::write(&path, "1").unwrap();

        let err = PidFile::acquire(&path).unwrap_err();
        assert!(matches!(err, PidFileError::AlreadyRunning { pid: 1 }));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "1");
    }

    #[test]
    fn stale_pid_is_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("status.pid");
        std::fs::write(&path, dead_pid().to_string()).unwrap();

        let _guard = PidFile::acquire(&path).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, std::process::id().to_string());
    }

    #[test]
    fn garbage_pid_file_is_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("status.pid");
        std::fs::write(&path, "not-a-pid\n").unwrap();

        let _guard = PidFile::acquire(&path).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, std::process::id().to_string());
    }

    #[test]
    fn pid_file_mode_is_0600() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("status.pid");

        let _guard = PidFile::acquire(&path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn negative_and_zero_pids_are_stale() {
        for bogus in ["-5", "0"] {
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path().join("status.pid");
            std::fs::write(&path, bogus).unwrap();

            let _guard = PidFile::acquire(&path).unwrap();
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                std::process::id().to_string()
            );
        }
    }
}
