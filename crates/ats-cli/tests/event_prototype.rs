use std::process::Command;

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

struct TmuxSession(String);

impl TmuxSession {
    fn new(name: &str) -> Self {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", name])
            .output();
        let ok = Command::new("tmux")
            .args(["new-session", "-d", "-s", name, "-x", "80", "-y", "24"])
            .output()
            .expect("spawn tmux")
            .status
            .success();
        assert!(ok, "failed to create detached tmux session");
        Self(name.to_string())
    }

    fn pane_id(&self) -> String {
        let out = Command::new("tmux")
            .args(["list-panes", "-t", &self.0, "-F", "#{pane_id}"])
            .output()
            .expect("list panes");
        String::from_utf8(out.stdout)
            .expect("utf8")
            .lines()
            .next()
            .expect("one pane")
            .to_string()
    }

    fn pane_border_style(&self, pane: &str) -> String {
        let out = Command::new("tmux")
            .args(["show-options", "-p", "-t", pane, "pane-border-style"])
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
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &self.0])
            .output();
    }
}

fn run_event(state: &str, pane: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["event", state])
        .env("TMUX_PANE", pane)
        .env_remove("TMUX")
        .output()
        .expect("run ats event")
}

#[test]
fn event_working_sets_pane_border_blue_on_target_pane_only() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let session = TmuxSession::new("ats-e2e-event");
    let pane = session.pane_id();

    let out = run_event("working", &pane);
    assert!(
        out.status.success(),
        "ats event working failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        session.pane_border_style(&pane),
        "pane-border-style fg=blue"
    );

    let out = run_event("attention", &pane);
    assert!(out.status.success());
    assert_eq!(
        session.pane_border_style(&pane),
        "pane-border-style fg=orange"
    );

    let out = run_event("idle", &pane);
    assert!(out.status.success());
    assert_eq!(
        session.pane_border_style(&pane),
        "",
        "idle must unset the pane-scoped style"
    );
}

#[test]
fn event_fails_open_outside_tmux() {
    let out = Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["event", "working"])
        .env_remove("TMUX_PANE")
        .env_remove("TMUX")
        .output()
        .expect("run ats event");
    assert!(
        out.status.success(),
        "hook-path commands must exit 0 even when no tmux target exists (fail-open)"
    );
}

#[test]
fn event_rejects_unknown_state_with_exit_zero() {
    let out = Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["event", "exploded"])
        .output()
        .expect("run ats event");
    assert!(
        out.status.success(),
        "unknown states must not break the hook path (fail-open)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown state"),
        "should hint at the problem on stderr: {stderr}"
    );
}
