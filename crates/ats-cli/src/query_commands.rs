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

    match std::fs::read_to_string(&log_file) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let filtered: Vec<&&str> = if let Some(lvl) = level {
                let needle = format!("\"level\":\"{lvl}\"");
                lines.iter().filter(|l| l.contains(&needle)).collect()
            } else {
                lines.iter().collect()
            };

            if tail {
                let start = if filtered.len() > 20 {
                    filtered.len() - 20
                } else {
                    0
                };
                for line in &filtered[start..] {
                    println!("{line}");
                }
            } else {
                for line in &filtered {
                    println!("{line}");
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to read log file: {e}");
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
