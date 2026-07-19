//! E2E and fault-injection integration tests (I-24).
//!
//! Uses the shared `tmux_driver` module to create isolated tmux sessions
//! and validate rendering behavior across the full lifecycle.

mod tmux_driver;

use std::process::Command;
use tmux_driver::{run_ats_event, tmux_available, tmux_supports_pane_scope, TmuxSession};

/// Full lifecycle E2E scenario (SPEC §20.3):
/// WORKING → ATTENTION → RESULT → ERROR → IDLE
#[test]
fn e2e_full_lifecycle() {
    if !tmux_available() {
        println!("SKIP: tmux not available");
        return;
    }
    if !tmux_supports_pane_scope() {
        println!("SKIP: tmux < 3.7");
        return;
    }

    let session = TmuxSession::new("ats-e2e-lifecycle");
    let pane = session.pane_id();
    let sibling = session.split();
    assert_ne!(pane, sibling, "pane and sibling must be distinct");

    // Step 1: WORKING → border should have color
    let out = run_ats_event("working", &pane);
    assert!(
        out.status.success(),
        "working failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        session
            .pane_border_style(&pane)
            .contains("pane-border-style"),
        "WORKING should set border style"
    );
    assert_eq!(
        session.pane_border_style(&sibling),
        "",
        "sibling pane must not be affected (SPEC §21 #5)"
    );

    // Step 2: ATTENTION → border should change
    let out = run_ats_event("attention", &pane);
    assert!(out.status.success());
    let attn_style = session.pane_border_style(&pane);
    assert!(
        attn_style.contains("pane-border-style"),
        "ATTENTION should set border style"
    );

    // Step 3: RESULT
    let out = run_ats_event("result", &pane);
    assert!(out.status.success());
    let result_style = session.pane_border_style(&pane);
    assert!(
        result_style.contains("pane-border-style"),
        "RESULT should set border style"
    );

    // Step 4: ERROR
    let out = run_ats_event("error", &pane);
    assert!(out.status.success());
    let error_style = session.pane_border_style(&pane);
    assert!(
        error_style.contains("pane-border-style"),
        "ERROR should set border style"
    );

    // Step 5: IDLE → border should reset
    let out = run_ats_event("idle", &pane);
    assert!(out.status.success());
    assert_eq!(
        session.pane_border_style(&pane),
        "",
        "IDLE must reset border style"
    );

    // Sibling should still be untouched
    assert_eq!(
        session.pane_border_style(&sibling),
        "",
        "sibling pane must never be affected"
    );
}

/// Multiple panes in the same session are isolated from each other.
#[test]
fn e2e_multi_pane_isolation() {
    if !tmux_available() {
        println!("SKIP: tmux not available");
        return;
    }
    if !tmux_supports_pane_scope() {
        println!("SKIP: tmux < 3.7");
        return;
    }

    let session = TmuxSession::new("ats-e2e-isolation");
    let pane_a = session.pane_id();
    let pane_b = session.split();

    // Set pane A to WORKING (blue)
    let out = run_ats_event("working", &pane_a);
    assert!(out.status.success());
    assert!(
        session
            .pane_border_style(&pane_a)
            .contains("pane-border-style"),
        "pane A should show WORKING"
    );
    assert_eq!(
        session.pane_border_style(&pane_b),
        "",
        "pane B should be untouched"
    );

    // Set pane B to ATTENTION (orange)
    let out = run_ats_event("attention", &pane_b);
    assert!(out.status.success());
    assert!(
        session
            .pane_border_style(&pane_b)
            .contains("pane-border-style"),
        "pane B should show ATTENTION"
    );
    assert!(
        session
            .pane_border_style(&pane_a)
            .contains("pane-border-style"),
        "pane A should still show WORKING"
    );

    // Reset pane A
    let out = run_ats_event("idle", &pane_a);
    assert!(out.status.success());
    assert_eq!(
        session.pane_border_style(&pane_a),
        "",
        "pane A should be reset"
    );
    assert!(
        session
            .pane_border_style(&pane_b)
            .contains("pane-border-style"),
        "pane B should still show ATTENTION"
    );
}

/// Fail-open: `ats event` returns exit 0 even when TMUX_PANE is not set.
#[test]
fn e2e_fail_open_no_tmux_pane() {
    let out = Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["event", "working"])
        .env_remove("TMUX_PANE")
        .env_remove("TMUX")
        .output()
        .expect("run ats event");
    assert!(
        out.status.success(),
        "fail-open: ats event must exit 0 even without TMUX_PANE (SPEC §15)"
    );
}

/// Fail-open: `ats reset` returns exit 0 even without tmux context.
#[test]
fn e2e_fail_open_reset_no_tmux() {
    let out = Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["reset"])
        .env_remove("TMUX_PANE")
        .env_remove("TMUX")
        .output()
        .expect("run ats reset");
    assert!(
        out.status.success(),
        "fail-open: ats reset must exit 0 even without tmux context"
    );
}

/// `ats reset` cleans up pane border styles.
#[test]
fn e2e_reset_clears_pane() {
    if !tmux_available() {
        println!("SKIP: tmux not available");
        return;
    }
    if !tmux_supports_pane_scope() {
        println!("SKIP: tmux < 3.7");
        return;
    }

    let session = TmuxSession::new("ats-e2e-reset");
    let pane = session.pane_id();

    // Set to WORKING first
    let out = run_ats_event("working", &pane);
    assert!(out.status.success());
    assert!(
        session
            .pane_border_style(&pane)
            .contains("pane-border-style"),
        "WORKING should set border style"
    );

    // Reset via CLI
    let out = Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["reset"])
        .env("TMUX_PANE", &pane)
        .output()
        .expect("run ats reset");
    assert!(
        out.status.success(),
        "ats reset failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Should be cleared
    assert_eq!(
        session.pane_border_style(&pane),
        "",
        "reset should clear border style"
    );
}

/// Standalone mode fallback: verify works when daemon is down.
#[test]
fn e2e_standalone_mode() {
    if !tmux_available() {
        println!("SKIP: tmux not available");
        return;
    }
    if !tmux_supports_pane_scope() {
        println!("SKIP: tmux < 3.7");
        return;
    }

    let session = TmuxSession::new("ats-e2e-standalone");
    let pane = session.pane_id();

    // Run without daemon — should fall back to standalone mode
    let out = run_ats_event("working", &pane);
    assert!(
        out.status.success(),
        "standalone event failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("daemon unreachable") || stderr.contains("standalone"),
        "should indicate standalone mode: {stderr}"
    );

    assert!(
        session
            .pane_border_style(&pane)
            .contains("pane-border-style"),
        "standalone mode should set border style"
    );
}

/// Unknown state returns exit 0 (fail-open) and does not crash.
#[test]
fn e2e_fault_unknown_state() {
    let out = Command::new(env!("CARGO_BIN_EXE_ats"))
        .args(["event", "nonsense_state_xyzzy"])
        .output()
        .expect("run ats event");
    assert!(
        out.status.success(),
        "unknown state must exit 0 (fail-open)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown state"),
        "should print diagnostic: {stderr}"
    );
}
