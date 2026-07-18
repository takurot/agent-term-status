use std::process::Command;

#[test]
fn version_flag_prints_name_and_crate_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_ats"))
        .arg("--version")
        .output()
        .expect("failed to run ats");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("stdout is not utf-8");
    assert!(
        stdout.starts_with("ats "),
        "version output should start with binary name: {stdout:?}"
    );
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output should contain crate version: {stdout:?}"
    );
}

#[cfg(unix)]
#[test]
fn version_works_when_invoked_as_agent_term_status_symlink() {
    let dir = std::env::temp_dir().join(format!("ats-symlink-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let link = dir.join("agent-term-status");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ats"), &link).expect("create symlink");

    let out = Command::new(&link)
        .arg("--version")
        .output()
        .expect("failed to run agent-term-status symlink");

    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_dir(&dir);

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("stdout is not utf-8");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output should contain crate version: {stdout:?}"
    );
}
