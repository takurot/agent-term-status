use std::io;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use ats_daemon::DaemonPaths;

// ---------------------------------------------------------------------------
// direct daemon management (start / stop / status)
// ---------------------------------------------------------------------------

fn daemon_binary() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("ats-daemon");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    if let Some(cargo_home) = option_env!("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".cargo")))
    {
        let candidate = cargo_home.join("bin").join("ats-daemon");
        if candidate.exists() {
            return Some(candidate);
        }
    }

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
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

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
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ats daemon: cannot start daemon: {e}");
            ExitCode::FAILURE
        }
    }
}

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

// ---------------------------------------------------------------------------
// launchd autostart integration (enable / disable / status)
// ---------------------------------------------------------------------------

const LAUNCHD_LABEL: &str = "ai.takurot.agent-term-status";

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn plist_path() -> PathBuf {
    home_dir()
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist"))
}

fn logs_dir() -> PathBuf {
    home_dir()
        .join(".local")
        .join("state")
        .join("agent-term-status")
        .join("logs")
}

fn daemon_binary_path() -> PathBuf {
    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ats-daemon"));
    if let Some(parent) = current_exe.parent() {
        let daemon_path = parent.join("ats-daemon");
        if daemon_path.exists() {
            return daemon_path;
        }
    }
    PathBuf::from("ats-daemon")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn run_enable() {
    let plist_path = plist_path();
    let daemon_binary = daemon_binary_path();
    let log_dir = logs_dir();

    if !daemon_binary.exists() {
        eprintln!("Daemon binary not found at: {}", daemon_binary.display());
        eprintln!("Make sure ats-daemon is on your PATH or in the same directory as ats.");
        std::process::exit(1);
    }

    let parent = plist_path.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap_or_else(|e| {
        eprintln!("Failed to create LaunchAgents directory: {e}");
        std::process::exit(1);
    });

    std::fs::create_dir_all(&log_dir).unwrap_or_else(|e| {
        eprintln!("Failed to create log directory: {e}");
        std::process::exit(1);
    });

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>Nice</key>
    <integer>10</integer>
    <key>StandardOutPath</key>
    <string>{stdout_log}</string>
    <key>StandardErrorPath</key>
    <string>{stderr_log}</string>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#,
        label = LAUNCHD_LABEL,
        binary = xml_escape(&daemon_binary.display().to_string()),
        stdout_log = xml_escape(&log_dir.join("daemon.stdout.log").display().to_string()),
        stderr_log = xml_escape(&log_dir.join("daemon.stderr.log").display().to_string()),
    );

    std::fs::write(&plist_path, &plist_content).unwrap_or_else(|e| {
        eprintln!("Failed to write plist: {e}");
        std::process::exit(1);
    });

    let load_status = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist_path)
        .status();

    match load_status {
        Ok(status) if status.success() => {
            println!("Daemon enabled. It will start on login and restart automatically.");
            println!("To start now: launchctl start {LAUNCHD_LABEL}");
        }
        Ok(status) => {
            eprintln!(
                "launchctl load failed with exit code: {}",
                status.code().unwrap_or(-1)
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run launchctl: {e}");
            std::process::exit(1);
        }
    }
}

pub fn run_disable() {
    let plist_path = plist_path();

    if !plist_path.exists() {
        println!("LaunchAgent plist not found. Daemon autostart is not configured.");
        return;
    }

    let unload_status = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&plist_path)
        .status();

    if let Err(e) = unload_status {
        eprintln!("Warning: launchctl unload failed: {e}");
    }

    if let Err(e) = std::fs::remove_file(&plist_path) {
        eprintln!("Failed to remove plist: {e}");
        std::process::exit(1);
    }

    let stop_status = Command::new("launchctl")
        .args(["stop", LAUNCHD_LABEL])
        .status();

    if let Err(e) = stop_status {
        eprintln!("Warning: launchctl stop failed: {e}");
    }

    println!("Daemon autostart disabled.");
}

pub fn run_launchd_status() {
    let output = Command::new("launchctl")
        .args(["list", LAUNCHD_LABEL])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains(LAUNCHD_LABEL) {
                let running = !stdout
                    .lines()
                    .any(|line| line.contains(LAUNCHD_LABEL) && line.trim().starts_with('-'));
                if running {
                    println!("Daemon is running (managed by launchd).");
                } else {
                    println!("Daemon is registered with launchd but not currently running.");
                }
            } else {
                println!("Daemon is not registered with launchd.");
            }
        }
        Ok(_) => {
            println!("Daemon is not registered with launchd.");
        }
        Err(e) => {
            eprintln!("Failed to check daemon status: {e}");
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- direct daemon tests --

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
        assert!(kill_daemon(&pid_path).is_err());
    }

    #[test]
    fn daemon_binary_path_resolves() {
        let _ = daemon_binary();
    }

    #[test]
    fn daemon_status_without_daemon_reports_not_running() {
        let tmp = tempfile::tempdir().unwrap();
        let paths =
            DaemonPaths::resolve_with_env(Some(tmp.path().to_str().unwrap()), Some(tmp.path()));
        assert!(!pid_is_alive(&paths.pid_path));
    }

    // -- launchd integration tests --

    #[test]
    fn plist_path_ends_with_label() {
        let path = plist_path();
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename.contains(LAUNCHD_LABEL));
        assert!(filename.ends_with(".plist"));
    }

    #[test]
    fn daemon_binary_path_not_empty() {
        let path = daemon_binary_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn daemon_launchd_status_detects_not_registered() {
        let output = Command::new("launchctl")
            .args(["list", "nonexistent.service.12345"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("nonexistent.service.12345"));
    }

    #[test]
    fn xml_escape_handles_special_chars() {
        assert_eq!(xml_escape("normal"), "normal");
        assert_eq!(xml_escape("a&b"), "a&amp;b");
        assert_eq!(xml_escape("a<b"), "a&lt;b");
        assert_eq!(xml_escape("a>b"), "a&gt;b");
        assert_eq!(xml_escape("&&"), "&amp;&amp;");
    }
}
