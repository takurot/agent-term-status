//! Tokio Unix Domain Socket server (SPEC §5.3.1–§5.3.2, §14.2).
//!
//! Accepts client connections, decodes length-prefixed frames, and
//! forwards raw payload bytes to the broker channel (broker logic lands
//! in I-14). Socket mode is restricted to `0600` at bind time.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::task::JoinSet;

use crate::framing::read_frame;

/// Pause after a failed `accept` before retrying.
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(50);

/// Tunables for the socket server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Maximum simultaneously served connections; excess connections are
    /// closed immediately.
    pub max_connections: usize,
    /// Per-read idle timeout; a connection that stays silent longer is
    /// closed.
    pub read_timeout: Duration,
    /// How long shutdown waits for in-flight connections to drain before
    /// aborting them.
    pub shutdown_grace: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: 64,
            read_timeout: Duration::from_secs(10),
            shutdown_grace: Duration::from_secs(5),
        }
    }
}

/// Bound Unix Domain Socket server.
#[derive(Debug)]
pub struct SocketServer {
    listener: UnixListener,
    socket_path: PathBuf,
    config: ServerConfig,
}

impl SocketServer {
    /// Binds the socket at `socket_path` and restricts its mode to `0600`.
    ///
    /// The chmod happens inside the bind step, before the server is
    /// handed out. The instant between `bind(2)` and `chmod(2)` is not
    /// exploitable: both supported parents (`$XDG_RUNTIME_DIR` per the
    /// XDG spec, or the `0700` state dir created by
    /// [`crate::DaemonPaths::ensure_parent_dirs`]) are user-only, so no
    /// other user can traverse to the socket. A process-wide `umask`
    /// dance was rejected: it races with concurrent file creation on
    /// other threads.
    ///
    /// As defense in depth, binding is refused when the parent directory
    /// is accessible by group or others.
    ///
    /// A pre-existing socket file is unlinked first; callers must ensure
    /// via [`crate::PidFile`] that no live daemon owns it.
    pub fn bind(socket_path: &Path, config: ServerConfig) -> std::io::Result<Self> {
        #[cfg(unix)]
        if let Some(parent) = socket_path.parent() {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(parent)?.permissions().mode();
            if mode & 0o077 != 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "socket parent directory {} is group/world accessible \
                         (mode {:o}); refusing to bind (SPEC §14.2)",
                        parent.display(),
                        mode & 0o777
                    ),
                ));
            }
        }

        match std::fs::remove_file(socket_path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }

        let listener = UnixListener::bind(socket_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(Self {
            listener,
            socket_path: socket_path.to_path_buf(),
            config,
        })
    }

    /// Bound socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Runs the accept loop until `shutdown` flips to `true`.
    ///
    /// On shutdown: stops accepting, drains in-flight connections within
    /// the configured grace period, unlinks the socket file.
    pub async fn run(self, events: mpsc::Sender<Vec<u8>>, mut shutdown: watch::Receiver<bool>) {
        // Unlinks the socket file even if this task panics mid-run.
        let _socket_guard = SocketFileGuard(self.socket_path.clone());
        let semaphore = Arc::new(Semaphore::new(self.config.max_connections));
        let mut connections: JoinSet<()> = JoinSet::new();

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                accepted = self.listener.accept() => {
                    let (stream, _addr) = match accepted {
                        Ok(conn) => conn,
                        Err(_e) => {
                            // Back off so a permanently broken listener fd
                            // cannot spin the loop at full CPU.
                            tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                            continue;
                        }
                    };
                    match Arc::clone(&semaphore).try_acquire_owned() {
                        Ok(permit) => {
                            let tx = events.clone();
                            let read_timeout = self.config.read_timeout;
                            connections.spawn(async move {
                                let _permit = permit;
                                handle_connection(stream, tx, read_timeout).await;
                            });
                        }
                        // At capacity: close the excess connection at once
                        // rather than queueing (hook clients fail open).
                        Err(_) => drop(stream),
                    }
                }
                Some(_finished) = connections.join_next(), if !connections.is_empty() => {}
            }
        }

        drop(self.listener);
        let drain = async { while connections.join_next().await.is_some() {} };
        if tokio::time::timeout(self.config.shutdown_grace, drain)
            .await
            .is_err()
        {
            connections.abort_all();
        }
    }
}

/// Removes the socket file when dropped (normal return or panic unwind).
struct SocketFileGuard(PathBuf);

impl Drop for SocketFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Serves one client connection until EOF, idle timeout, framing error,
/// or broker channel closure. Framing errors carry sizes only (privacy
/// invariant, SPEC §14.2).
async fn handle_connection(
    mut stream: UnixStream,
    events: mpsc::Sender<Vec<u8>>,
    read_timeout: Duration,
) {
    loop {
        match tokio::time::timeout(read_timeout, read_frame(&mut stream)).await {
            // Idle timeout: close the connection.
            Err(_elapsed) => break,
            // Clean EOF.
            Ok(Ok(None)) => break,
            Ok(Ok(Some(payload))) => {
                if events.send(payload).await.is_err() {
                    break;
                }
            }
            // Oversized or I/O error: drop this connection; the listener
            // stays available for other clients.
            Ok(Err(_frame_error)) => break,
        }
    }
}
