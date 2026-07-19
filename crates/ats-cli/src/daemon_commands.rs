use std::path::PathBuf;
use std::process::Command;

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
        binary = daemon_binary.display(),
        stdout_log = log_dir.join("daemon.stdout.log").display(),
        stderr_log = log_dir.join("daemon.stderr.log").display(),
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

pub fn run_status() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_path_ends_with_label() {
        let path = plist_path();
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename.contains(LAUNCHD_LABEL));
        assert!(filename.ends_with(".plist"));
    }

    #[test]
    fn daemon_binary_path_resolves() {
        let path = daemon_binary_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn daemon_status_detects_not_registered() {
        let output = Command::new("launchctl")
            .args(["list", "nonexistent.service.12345"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("nonexistent.service.12345"));
    }
}
