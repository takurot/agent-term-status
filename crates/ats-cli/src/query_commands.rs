use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};

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

pub fn run_status(session_id: Option<&str>) {
    if !socket_client::daemon_is_reachable() {
        println!("Daemon is not running.");
        return;
    }

    if let Some(id) = session_id {
        println!("Session: {id}");
        println!("State: unknown (query protocol not yet implemented)");
    } else {
        println!("Daemon is running.");
        println!("Use --session <id> for per-session status.");
    }
}

pub fn run_list(json: bool) {
    if !socket_client::daemon_is_reachable() {
        if json {
            println!("{{\"sessions\": [], \"error\": \"daemon not reachable\"}}");
        } else {
            println!("Daemon is not running. No sessions to list.");
        }
        return;
    }

    if json {
        println!("{{\"sessions\": [], \"note\": \"query protocol not yet implemented\"}}");
    } else {
        println!("Daemon is running.");
        println!("Session listing via query protocol not yet implemented.");
    }
}

pub fn run_logs(tail: bool, level: Option<&str>) {
    let log_dir = logs_dir();
    let log_file = log_dir.join("ats-daemon.log");

    if !log_file.exists() {
        println!(
            "Log file not found at {}. Daemon may not have started yet.",
            log_file.display()
        );
        return;
    }

    let file = match File::open(&log_file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open log file: {e}");
            return;
        }
    };

    let reader = BufReader::new(file);
    let needle = level.map(|lvl| format!("\"level\":\"{lvl}\""));

    if tail {
        let mut ring: VecDeque<String> = VecDeque::with_capacity(1000);
        for line_result in reader.lines() {
            if let Ok(line) = line_result {
                if let Some(ref n) = needle {
                    if line.contains(n.as_str()) {
                        if ring.len() == 1000 {
                            ring.pop_front();
                        }
                        ring.push_back(line);
                    }
                } else {
                    if ring.len() == 1000 {
                        ring.pop_front();
                    }
                    ring.push_back(line);
                }
            }
        }
        for line in &ring {
            println!("{line}");
        }
    } else {
        for line_result in reader.lines() {
            if let Ok(line) = line_result {
                if let Some(ref n) = needle {
                    if line.contains(n.as_str()) {
                        println!("{line}");
                    }
                } else {
                    println!("{line}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_without_daemon_does_not_crash() {
        run_status(None);
    }

    #[test]
    fn list_json_produces_valid_json() {
        run_list(true);
    }

    #[test]
    fn list_without_daemon_does_not_crash() {
        run_list(false);
    }
}
