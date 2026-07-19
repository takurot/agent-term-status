use ats_config::ConfigPaths;
use ats_daemon::DaemonPaths;

use crate::socket_client;

fn logs_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".local")
        .join("state")
        .join("agent-term-status")
        .join("logs")
}

fn check(label: &str, ok: bool, detail: &str) {
    let status = if ok { "GREEN" } else { "RED" };
    println!("  [{status}] {label}: {detail}");
}

fn check_amber(label: &str, detail: &str) {
    println!("  [AMBER] {label}: {detail}");
}

pub fn run_doctor(fix: bool) {
    println!("ats doctor");
    println!();

    check_daemon();
    check_socket_mode();
    check_paths_exist(fix);
    check_log_dir_writable();
    check_config_valid();
    check_themes_load();
    check_tmux();

    if fix {
        println!();
        println!("Auto-repair: directory creation attempted where possible.");
    }
}

fn check_daemon() {
    if socket_client::daemon_is_reachable() {
        check("Daemon", true, "running and reachable");
    } else {
        check(
            "Daemon",
            false,
            "not running. Start with: ats daemon enable && ats-daemon",
        );
    }
}

fn check_socket_mode() {
    let socket_path = DaemonPaths::resolve().socket_path;
    if !socket_path.exists() {
        check_amber("Socket mode", "socket file does not exist");
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(&socket_path) {
            Ok(meta) => {
                let mode = meta.permissions().mode();
                if mode & 0o777 == 0o600 {
                    check("Socket mode", true, "0600 (user read/write only)");
                } else {
                    check(
                        "Socket mode",
                        false,
                        &format!("{:o} (should be 600)", mode & 0o777),
                    );
                }
            }
            Err(e) => check("Socket mode", false, &format!("cannot stat: {e}")),
        }
    }
}

fn check_paths_exist(fix: bool) {
    let paths = ConfigPaths::resolve(None);
    let daemon_paths = DaemonPaths::resolve();

    let dirs = vec![
        ("Config dir", paths.user_config_dir.clone()),
        ("State dir", paths.user_state_dir.clone()),
        (
            "Socket dir",
            daemon_paths
                .socket_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default(),
        ),
    ];

    for (label, dir) in &dirs {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if dir.exists() {
            check(label, true, &format!("{}", dir.display()));
        } else if fix {
            match std::fs::create_dir_all(dir) {
                Ok(()) => check(label, true, &format!("{} (created)", dir.display())),
                Err(e) => check(
                    label,
                    false,
                    &format!("{} (cannot create: {e})", dir.display()),
                ),
            }
        } else {
            check(label, false, &format!("{} (missing)", dir.display()));
        }
    }
}

fn check_log_dir_writable() {
    let log_dir = logs_dir();
    if !log_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            check(
                "Log dir writable",
                false,
                &format!("{} (cannot create: {e})", log_dir.display()),
            );
            return;
        }
    }

    let test_file = log_dir.join(".doctor-write-test");
    match std::fs::write(&test_file, b"test") {
        Ok(()) => {
            let _ = std::fs::remove_file(&test_file);
            check("Log dir writable", true, &format!("{}", log_dir.display()));
        }
        Err(e) => check(
            "Log dir writable",
            false,
            &format!("{} ({e})", log_dir.display()),
        ),
    }
}

fn check_config_valid() {
    let paths = ConfigPaths::resolve(None);
    let config_file = paths.user_config_file();
    if !config_file.exists() {
        check_amber("Config", "no user config (defaults apply)");
        return;
    }

    match std::fs::read_to_string(config_file) {
        Ok(content) => match serde_yaml::from_str::<ats_config::Config>(&content) {
            Ok(config) => match config.validate() {
                Ok(()) => check(
                    "Config",
                    true,
                    &format!("valid ({})", config_file.display()),
                ),
                Err(errors) => check("Config", false, &format!("invalid: {}", errors.join("; "))),
            },
            Err(e) => check("Config", false, &format!("parse error: {e}")),
        },
        Err(e) => check("Config", false, &format!("cannot read: {e}")),
    }
}

fn check_themes_load() {
    let names = ats_config::theme::Theme::bundle_names();
    let mut all_ok = true;
    let mut detail = String::new();

    for name in &names {
        match ats_config::theme::Theme::load_bundled(name) {
            Ok(theme) => match theme.validate() {
                Ok(()) => {}
                Err(e) => {
                    all_ok = false;
                    detail.push_str(&format!("{name}: {e}; "));
                }
            },
            Err(e) => {
                all_ok = false;
                detail.push_str(&format!("{name}: {e}; "));
            }
        }
    }

    if all_ok {
        check(
            "Themes",
            true,
            &format!("{} bundled themes load", names.len()),
        );
    } else {
        check("Themes", false, &detail);
    }
}

fn check_tmux() {
    let found = std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if found {
        let output = std::process::Command::new("tmux")
            .args(["-V"])
            .output()
            .ok();
        let version = output
            .as_ref()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        check("tmux", true, &format!("found ({version})"));
    } else {
        check("tmux", false, "not found in PATH");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_runs_without_crashing() {
        run_doctor(false);
    }

    #[test]
    fn doctor_with_fix_runs_without_crashing() {
        run_doctor(true);
    }
}
