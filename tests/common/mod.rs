use std::process::Command;

pub struct TmuxDriver {
    server_name: String,
    session_name: String,
}

impl TmuxDriver {
    #[allow(dead_code)]
    pub fn new(prefix: &str) -> Self {
        let server_name = format!("ats-test-{}-{}", prefix, std::process::id());
        let session_name = format!("{prefix}-{}", std::process::id());

        let _ = Command::new("tmux")
            .args(["-L", &server_name, "kill-server"])
            .output();

        let ok = Command::new("tmux")
            .args([
                "-L",
                &server_name,
                "new-session",
                "-d",
                "-s",
                &session_name,
                "-x",
                "80",
                "-y",
                "24",
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !ok {
            eprintln!("TmuxDriver: failed to create test session");
        }

        Self {
            server_name,
            session_name,
        }
    }

    #[allow(dead_code)]
    pub fn session_name(&self) -> &str {
        &self.session_name
    }

    #[allow(dead_code)]
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    #[allow(dead_code)]
    pub fn create_pane(&self) -> Option<String> {
        let ok = Command::new("tmux")
            .args([
                "-L",
                &self.server_name,
                "split-window",
                "-h",
                "-t",
                &self.session_name,
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if ok {
            self.pane_ids().pop()
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn pane_ids(&self) -> Vec<String> {
        Command::new("tmux")
            .args([
                "-L",
                &self.server_name,
                "list-panes",
                "-t",
                &self.session_name,
                "-F",
                "#{pane_id}",
            ])
            .output()
            .map(|o| {
                String::from_utf8(o.stdout)
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn first_pane(&self) -> Option<String> {
        self.pane_ids().into_iter().next()
    }

    #[allow(dead_code)]
    pub fn pane_format(&self, pane: &str, format: &str) -> String {
        Command::new("tmux")
            .args([
                "-L",
                &self.server_name,
                "display-message",
                "-p",
                "-t",
                pane,
                "-F",
                format,
            ])
            .output()
            .map(|o| String::from_utf8(o.stdout).unwrap_or_default().trim().to_string())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn set_pane_option(&self, pane: &str, option: &str, value: &str) -> bool {
        Command::new("tmux")
            .args([
                "-L",
                &self.server_name,
                "set-option",
                "-p",
                "-t",
                pane,
                option,
                value,
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn show_pane_option(&self, pane: &str, option: &str) -> String {
        Command::new("tmux")
            .args([
                "-L",
                &self.server_name,
                "show-options",
                "-p",
                "-t",
                pane,
                option,
            ])
            .output()
            .map(|o| String::from_utf8(o.stdout).unwrap_or_default().trim().to_string())
            .unwrap_or_default()
    }
}

impl Drop for TmuxDriver {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["-L", &self.server_name, "kill-server"])
            .output();
    }
}

#[allow(dead_code)]
pub fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

use ats_core::NormalizedEvent;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HookFixture {
    hook_event_name: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
}

#[allow(dead_code)]
pub fn load_claude_fixtures() -> Vec<NormalizedEvent> {
    let fixture_dir = std::path::Path::new("tests/fixtures/claude");
    if !fixture_dir.is_dir() {
        return vec![];
    }

    let mut events = Vec::new();
    if let Ok(entries) = std::fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Ok(event) = serde_json::from_value::<NormalizedEvent>(value.clone())
                        {
                            events.push(event);
                        }
                    }
                }
            }
        }
    }

    events
}

#[allow(dead_code)]
pub fn fixture_count() -> usize {
    let fixture_dir = std::path::Path::new("tests/fixtures/claude");
    if !fixture_dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(fixture_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext == "json")
                })
                .count()
        })
        .unwrap_or(0)
}
