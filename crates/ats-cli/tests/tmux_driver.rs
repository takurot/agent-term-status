//! Shared tmux driver for E2E and integration tests.
//!
//! Creates isolated detached tmux sessions using unique session names. Does
//! NOT use `-L` (custom socket) — instead relies on session name isolation.
//! The `TMUX_TMPDIR` env var is set consistently for both the test harness
//! and the `ats` binary subprocess.

use std::process::Command;

pub fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn tmux_supports_pane_scope() -> bool {
    let Ok(out) = Command::new("tmux").arg("-V").output() else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let Some(start) = raw.find(|c: char| c.is_ascii_digit()) else {
        return false;
    };
    let tail = &raw[start..];
    let end = tail
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(tail.len());
    let mut parts = tail[..end].split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor) >= (3, 7)
}

pub struct TmuxSession {
    name: String,
}

impl TmuxSession {
    pub fn new(prefix: &str) -> Self {
        let name = format!("{prefix}-{}", std::process::id());
        let _ = tmux(&["kill-session", "-t", &name]);
        let ok = tmux(&["new-session", "-d", "-s", &name, "-x", "80", "-y", "24"]);
        assert!(ok, "failed to create detached tmux session '{name}'");
        Self { name }
    }

    pub fn split(&self) -> String {
        let ok = tmux(&["split-window", "-h", "-t", &self.name]);
        assert!(ok, "failed to split tmux window in session '{}'", self.name);
        self.pane_ids().pop().expect("sibling pane")
    }

    pub fn pane_ids(&self) -> Vec<String> {
        let out = Command::new("tmux")
            .args(["list-panes", "-t", &self.name, "-F", "#{pane_id}"])
            .output()
            .expect("list panes");
        String::from_utf8(out.stdout)
            .expect("utf8")
            .lines()
            .map(str::to_string)
            .collect()
    }

    pub fn pane_id(&self) -> String {
        self.pane_ids().into_iter().next().expect("one pane")
    }

    pub fn pane_border_style(&self, pane: &str) -> String {
        let out = Command::new("tmux")
            .args(["show-options", "-p", "-t", pane, "pane-border-style"])
            .output()
            .expect("show options");
        String::from_utf8(out.stdout)
            .expect("utf8")
            .trim()
            .to_string()
    }

    #[allow(dead_code)]
    pub fn pane_border_format(&self, pane: &str) -> String {
        let out = Command::new("tmux")
            .args(["show-options", "-p", "-t", pane, "pane-border-format"])
            .output()
            .expect("show options");
        String::from_utf8(out.stdout)
            .expect("utf8")
            .trim()
            .to_string()
    }
}

impl Drop for TmuxSession {
    fn drop(&mut self) {
        let _ = tmux(&["kill-session", "-t", &self.name]);
    }
}

fn tmux(args: &[&str]) -> bool {
    Command::new("tmux")
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `ats event <state>` with TMUX_PANE env. Both the test session and
/// the ats binary use the default tmux socket (same server).
pub fn run_ats_event(state: &str, pane: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["event", state])
        .env("TMUX_PANE", pane)
        .output()
        .expect("run ats event")
}
