use std::io;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use ats_daemon::DaemonPaths;

/// Finds the `ats-daemon` binary path.
///
/// Searches: next to current exe → `$CARGO_HOME/bin` → `$PATH`.
/// The compile-time env `CARGO_BIN_EXE_ats-daemon` is not always available
/// after installation.
fn daemon_binary() -> Option<PathBuf> {
    // Same directory as the current executable (works for bundled installs).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("ats-daemon");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // $CARGO_HOME/bin (works for `cargo install` patterns).
    if let Some(cargo_home) = option_env!("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".cargo")))
    {
        let candidate = cargo_home.join("bin").join("ats-daemon");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // $PATH fallback.
    if let Ok(paths) = std::env::var("PATH") {
        for dir in paths.split(':') {
            let candidate = Path::new(dir).join("ats-daemon");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

fn pid_is_alive(pid_path: &Path) -> bool {
    let content = match std::fs::read_to_string(pid_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let pid: i32 = match content.trim().parse() {
        Ok(p) if p > 0 => p,
        _ => return false,
    };
    // Signal 0 checks existence without sending.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn kill_daemon(pid_path: &Path) -> io::Result<()> {
    if !pid_path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(pid_path)?;
    let pid: i32 = content
        .trim()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "malformed PID file"))?;
    if pid <= 0 {
        let _ = std::fs::remove_file(pid_path);
        return Ok(());
    }
    unsafe {
        if libc::kill(pid, libc::SIGTERM) != 0 {
            let err = io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::ESRCH) => {
                    let _ = std::fs::remove_file(pid_path);
                    return Ok(());
                }
                _ => return Err(err),
            }
        }
    }
    Ok(())
}

/// Polls the socket up to `timeout` for connectivity.
fn socket_reachable(socket_path: &Path, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    loop {
        match UnixStream::connect(socket_path) {
            Ok(_) => return true,
            Err(_) if start.elapsed() >= timeout => return false,
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// `ats daemon start [--foreground]`
pub fn start(foreground: bool) -> ExitCode {
    let paths = DaemonPaths::resolve();

    if pid_is_alive(&paths.pid_path) {
        eprintln!("ats daemon: daemon is already running");
        return ExitCode::SUCCESS;
    }

    let Some(binary) = daemon_binary() else {
        eprintln!("ats daemon: cannot find ats-daemon binary");
        return ExitCode::FAILURE;
    };

    let mut cmd = Command::new(&binary);

    if foreground {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

        unsafe {
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
            {
                cmd.pre_exec(|| {
                    libc::setpgid(0, 0);
                    Ok(())
                });
            }
            #[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")))]
            {
                cmd.pre_exec(|| {
                    if libc::daemon(1, 0) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
    }

    match cmd.spawn() {
        Ok(mut child) => {
            if foreground {
                if let Err(e) = child.wait() {
                    eprintln!("ats daemon: daemon exited with error: {e}");
                    return ExitCode::FAILURE;
                }
            } else {
                eprintln!("ats daemon: daemon started (PID: {})", child.id());
                // Don't wait on background daemon.
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ats daemon: cannot start daemon: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `ats daemon stop`
pub fn stop() -> ExitCode {
    let paths = DaemonPaths::resolve();

    match kill_daemon(&paths.pid_path) {
        Ok(()) => {
            eprintln!("ats daemon: stop signal sent");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ats daemon: cannot stop daemon: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `ats daemon status [--json]`
pub fn status(json: bool) -> ExitCode {
    let paths = DaemonPaths::resolve();

    let pid_alive = pid_is_alive(&paths.pid_path);
    let sock_alive = socket_reachable(&paths.socket_path, Duration::from_secs(2));
    let healthy = pid_alive && sock_alive;

    if json {
        let payload = serde_json::json!({
            "running": healthy,
            "pid_alive": pid_alive,
            "socket_reachable": sock_alive,
            "socket_path": paths.socket_path,
            "pid_path": paths.pid_path,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
    } else if healthy {
        println!(
            "ats daemon: running (socket: {})",
            paths.socket_path.display()
        );
    } else if pid_alive {
        println!("ats daemon: PID alive but socket unreachable");
    } else {
        println!("ats daemon: not running");
    }

    if healthy {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_is_alive_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let pid_path = tmp.path().join("nonexistent.pid");
        assert!(!pid_is_alive(&pid_path));
    }

    #[test]
    fn pid_is_alive_negative_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let pid_path = tmp.path().join("bad.pid");
        std::fs::write(&pid_path, "-1\n").unwrap();
        assert!(!pid_is_alive(&pid_path));
    }

    #[test]
    fn kill_daemon_no_file_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let pid_path = tmp.path().join("nonexistent.pid");
        assert!(kill_daemon(&pid_path).is_ok());
    }

    #[test]
    fn kill_daemon_garbage_file() {
        let tmp = tempfile::tempdir().unwrap();
        let pid_path = tmp.path().join("garbage.pid");
        std::fs::write(&pid_path, "not a pid").unwrap();
        // Should fail because it's not a valid PID, but remove the file first.
        assert!(kill_daemon(&pid_path).is_err());
    }

    #[test]
    fn daemon_binary_path_resolves() {
        // In debug builds executed via cargo, the binary may exist in target/debug.
        // This test just confirms the resolution logic doesn't panic.
        let _ = daemon_binary();
    }

    #[test]
    fn daemon_status_without_daemon_reports_not_running() {
        // Create a temp PID path that doesn't exist.
        let tmp = tempfile::tempdir().unwrap();
        let paths =
            DaemonPaths::resolve_with_env(Some(tmp.path().to_str().unwrap()), Some(tmp.path()));
        assert!(!pid_is_alive(&paths.pid_path));
    }
}
