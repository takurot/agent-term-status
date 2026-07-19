use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use ats_daemon::framing::write_frame_sync;
use ats_daemon::DaemonPaths;

pub fn daemon_socket_path() -> PathBuf {
    DaemonPaths::resolve().socket_path
}

pub fn send_frame_to_daemon(payload: &[u8]) -> Result<(), String> {
    let socket_path = daemon_socket_path();

    let mut stream = UnixStream::connect(&socket_path).map_err(|e| {
        format!(
            "failed to connect to daemon at {}: {e}",
            socket_path.display()
        )
    })?;

    write_frame_sync(&mut stream, payload).map_err(|e| format!("failed to write frame: {e}"))?;

    Ok(())
}

#[allow(dead_code)]
pub fn daemon_is_reachable() -> bool {
    UnixStream::connect(daemon_socket_path()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_is_absolute() {
        let path = daemon_socket_path();
        assert!(path.is_absolute() || path.starts_with("/"));
    }
}
