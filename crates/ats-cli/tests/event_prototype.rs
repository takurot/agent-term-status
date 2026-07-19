use std::process::Command;

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Pane-scoped options require tmux >= 3.7 (see docs/spikes/tmux-pane-safety.md);
/// older versions leak the option to window scope and the prototype refuses.
fn tmux_supports_pane_scope() -> bool {
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

struct TmuxSession(String);

impl TmuxSession {
    fn new(prefix: &str) -> Self {
        let name = format!("{prefix}-{}", std::process::id());
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &name])
            .output();
        let ok = Command::new("tmux")
            .args(["new-session", "-d", "-s", &name, "-x", "80", "-y", "24"])
            .output()
            .expect("spawn tmux")
            .status
            .success();
        assert!(ok, "failed to create detached tmux session");
        Self(name)
    }

    fn split(&self) -> String {
        let ok = Command::new("tmux")
            .args(["split-window", "-h", "-t", &self.0])
            .output()
            .expect("split window")
            .status
            .success();
        assert!(ok, "failed to split tmux window");
        self.pane_ids().pop().expect("sibling pane")
    }

    fn pane_ids(&self) -> Vec<String> {
        let out = Command::new("tmux")
            .args(["list-panes", "-t", &self.0, "-F", "#{pane_id}"])
            .output()
            .expect("list panes");
        String::from_utf8(out.stdout)
            .expect("utf8")
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn pane_id(&self) -> String {
        self.pane_ids().into_iter().next().expect("one pane")
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
    // TMUX is deliberately NOT overridden: production inherits the ambient
    // socket env, and so must the test to talk to the same tmux server.
    Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["event", state])
        .env("TMUX_PANE", pane)
        .output()
        .expect("run ats event")
}

#[test]
fn event_working_sets_pane_border_blue_on_target_pane_only() {
    if !tmux_available() {
        println!("skipping: tmux not available");
        return;
    }
    if !tmux_supports_pane_scope() {
        println!("skipping: tmux < 3.7 leaks pane options to window scope");
        return;
    }
    let session = TmuxSession::new("ats-e2e-event");
    let pane = session.pane_id();
    let sibling = session.split();
    assert_ne!(pane, sibling);

    let out = run_event("working", &pane);
    assert!(
        out.status.success(),
        "ats event working failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let style = session.pane_border_style(&pane);
    assert!(
        style.contains("pane-border-style"),
        "border style should be set for pane {pane}: got '{style}'"
    );
    assert_eq!(
        session.pane_border_style(&sibling),
        "",
        "sibling pane must never be touched (SPEC §21 #5)"
    );

    let out = run_event("attention", &pane);
    assert!(out.status.success());
    let attn_style = session.pane_border_style(&pane);
    assert!(
        attn_style.contains("pane-border-style"),
        "border style should be set for pane {pane} attention: got '{attn_style}'"
    );
    assert_eq!(session.pane_border_style(&sibling), "");

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
