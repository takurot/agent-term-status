use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use ats_daemon::DaemonPaths;

pub fn daemon_socket_path() -> PathBuf {
    DaemonPaths::resolve().socket_path
}

#[allow(dead_code)]
pub fn send_frame_to_daemon(payload: &[u8]) -> Result<(), String> {
    let socket_path = daemon_socket_path();

    let mut stream = UnixStream::connect(&socket_path).map_err(|e| {
        format!(
            "failed to connect to daemon at {}: {e}",
            socket_path.display()
        )
    })?;

    let len = payload.len() as u32;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| format!("failed to write frame header: {e}"))?;
    stream
        .write_all(payload)
        .map_err(|e| format!("failed to write frame payload: {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("failed to flush: {e}"))?;

    Ok(())
}

pub fn daemon_is_reachable() -> bool {
    UnixStream::connect(daemon_socket_path()).is_ok()
}
