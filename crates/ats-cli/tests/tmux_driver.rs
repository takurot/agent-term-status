//! Shared tmux driver for E2E and integration tests.
//!
//! Creates isolated detached tmux sessions using unique session names.
//! Each TmuxSession creates a temporary TMUX_TMPDIR to completely isolate
//! the tmux server from the user's default tmux instance.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

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
    tmpdir: TempDir,
}

impl TmuxSession {
    pub fn new(prefix: &str) -> Self {
        let tmpdir = TempDir::new().expect("create temp dir for tmux socket");
        let name = format!("{prefix}-{}", std::process::id());
        let _ = tmux_in_dir(&["kill-session", "-t", &name], tmpdir.path());
        let ok = tmux_in_dir(
            &["new-session", "-d", "-s", &name, "-x", "80", "-y", "24"],
            tmpdir.path(),
        );
        assert!(ok, "failed to create detached tmux session '{name}'");
        Self { name, tmpdir }
    }

    fn tmux_cmd(&self) -> Command {
        let mut cmd = Command::new("tmux");
        cmd.env("TMUX_TMPDIR", self.tmpdir.path());
        cmd
    }

    pub fn tmpdir_path(&self) -> &Path {
        self.tmpdir.path()
    }

    pub fn split(&self) -> String {
        let output = self
            .tmux_cmd()
            .args(["split-window", "-h", "-t", &self.name])
            .output()
            .expect("tmux split-window");
        assert!(
            output.status.success(),
            "failed to split tmux window in session '{}'",
            self.name
        );
        self.pane_ids().pop().expect("sibling pane")
    }

    pub fn pane_ids(&self) -> Vec<String> {
        let out = self
            .tmux_cmd()
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
        let out = self
            .tmux_cmd()
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
        let out = self
            .tmux_cmd()
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
        let _ = tmux_in_dir(&["kill-session", "-t", &self.name], self.tmpdir.path());
    }
}

fn tmux_in_dir(args: &[&str], tmpdir: &Path) -> bool {
    Command::new("tmux")
        .args(args)
        .env("TMUX_TMPDIR", tmpdir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `ats event <state>` with TMUX_PANE env, using an isolated tmux socket.
pub fn run_ats_event(state: &str, pane: &str, tmpdir: &Path) -> std::process::Output {
    Command::new(option_env!("CARGO_BIN_EXE_ats").unwrap_or("ats"))
        .args(["event", state])
        .env("TMUX_PANE", pane)
        .env("TMUX_TMPDIR", tmpdir)
        .output()
        .expect("run ats event")
}
