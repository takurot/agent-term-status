//! Integration tests for the `ats-daemon` socket server (I-13 DoD).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ats_daemon::{
    read_frame, write_frame, DaemonPaths, PidFile, ServerConfig, SocketServer, MAX_FRAME_BYTES,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, watch};

/// Short socket path inside a tempdir (macOS caps `sun_path` at 104 bytes).
fn socket_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("s.sock")
}

struct RunningServer {
    events: mpsc::Receiver<Vec<u8>>,
    shutdown: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

fn start_server(path: &Path, config: ServerConfig) -> RunningServer {
    let server = SocketServer::bind(path, config).expect("bind server");
    let (tx, rx) = mpsc::channel(128);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let task = tokio::spawn(server.run(tx, shutdown_rx));
    RunningServer {
        events: rx,
        shutdown: shutdown_tx,
        task,
    }
}

async fn recv_event(rx: &mut mpsc::Receiver<Vec<u8>>) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("event channel closed")
}

#[tokio::test]
async fn multi_client_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let mut server = start_server(&path, ServerConfig::default());

    let mut expected = Vec::new();
    let mut clients = Vec::new();
    for i in 0..5 {
        let payload = format!(r#"{{"client":{i},"event":"agent.working"}}"#).into_bytes();
        expected.push(payload.clone());
        let path = path.clone();
        clients.push(tokio::spawn(async move {
            let mut stream = UnixStream::connect(&path).await.expect("connect");
            write_frame(&mut stream, &payload).await.expect("send");
        }));
    }
    for c in clients {
        c.await.unwrap();
    }

    let mut received = Vec::new();
    for _ in 0..5 {
        received.push(recv_event(&mut server.events).await);
    }
    received.sort();
    expected.sort();
    assert_eq!(received, expected);

    server.shutdown.send(true).unwrap();
    server.task.await.unwrap();
}

#[tokio::test]
async fn single_client_multiple_frames_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let mut server = start_server(&path, ServerConfig::default());

    let mut stream = UnixStream::connect(&path).await.unwrap();
    for i in 0..3 {
        let payload = format!(r#"{{"seq":{i}}}"#).into_bytes();
        write_frame(&mut stream, &payload).await.unwrap();
    }

    for i in 0..3 {
        let got = recv_event(&mut server.events).await;
        assert_eq!(got, format!(r#"{{"seq":{i}}}"#).into_bytes());
    }

    drop(stream);
    server.shutdown.send(true).unwrap();
    server.task.await.unwrap();
}

#[tokio::test]
async fn socket_mode_is_0600_at_bind_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let _server = SocketServer::bind(&path, ServerConfig::default()).unwrap();

    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "socket must be user-only (SPEC §14.2)");
}

#[tokio::test]
async fn oversized_payload_rejected_and_socket_stays_available() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let mut server = start_server(&path, ServerConfig::default());

    // Client A declares an oversized frame; server must close the
    // connection without reading the payload.
    let mut bad = UnixStream::connect(&path).await.unwrap();
    let declared: u32 = MAX_FRAME_BYTES + 1;
    bad.write_all(&declared.to_be_bytes()).await.unwrap();
    bad.flush().await.unwrap();

    let mut buf = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(5), bad.read(&mut buf))
        .await
        .expect("server should close oversized connection promptly")
        .unwrap_or(0);
    assert_eq!(n, 0, "server must close the oversized connection");

    // Client B still gets served.
    let mut good = UnixStream::connect(&path).await.unwrap();
    write_frame(&mut good, br#"{"ok":true}"#).await.unwrap();
    let got = recv_event(&mut server.events).await;
    assert_eq!(got, br#"{"ok":true}"#.to_vec());

    drop(good);
    server.shutdown.send(true).unwrap();
    server.task.await.unwrap();
}

#[tokio::test]
async fn connect_to_dead_socket_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);

    // Bind then drop the listener, leaving a stale socket file behind.
    {
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        drop(listener);
    }
    assert!(path.exists(), "stale socket file should remain");

    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(2), UnixStream::connect(&path)).await;
    let outcome = result.expect("connect must not hang");
    match outcome {
        // Expected: prompt ECONNREFUSED.
        Err(_) => {}
        // Rare macOS race under parallel tests: a concurrently spawned
        // child process (from the PID-file tests) can inherit the
        // listener fd in the window between socket() and FD_CLOEXEC,
        // briefly keeping the dead listener alive. Nobody will ever
        // accept: the connection must still terminate promptly.
        Ok(mut stream) => {
            let mut buf = [0u8; 1];
            let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf))
                .await
                .expect("dead socket connection must not hang")
                .unwrap_or(0);
            assert_eq!(n, 0, "dead socket must yield EOF, not data");
        }
    }
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "failure must be prompt, not a retry loop"
    );
}

#[tokio::test]
async fn stale_socket_file_is_replaced_on_bind() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    {
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        drop(listener);
    }

    let mut server = start_server(&path, ServerConfig::default());
    let mut stream = UnixStream::connect(&path).await.unwrap();
    write_frame(&mut stream, br#"{"recovered":true}"#)
        .await
        .unwrap();
    let got = recv_event(&mut server.events).await;
    assert_eq!(got, br#"{"recovered":true}"#.to_vec());

    drop(stream);
    server.shutdown.send(true).unwrap();
    server.task.await.unwrap();
}

#[tokio::test]
async fn kill_dash_nine_recovery_stale_pid_and_socket() {
    // Simulates the post-`kill -9` state: stale PID file + stale socket.
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("d.pid");
    let sock = socket_path(&dir);

    let child = std::process::Command::new("true").spawn().unwrap();
    let dead = child.id() as i32;
    let mut child = child;
    child.wait().unwrap();
    std::fs::write(&pid_path, dead.to_string()).unwrap();
    {
        let listener = tokio::net::UnixListener::bind(&sock).unwrap();
        drop(listener);
    }

    let guard = PidFile::acquire(&pid_path).expect("stale PID file must be replaced");
    assert_eq!(
        std::fs::read_to_string(&pid_path).unwrap(),
        std::process::id().to_string()
    );

    let mut server = start_server(&sock, ServerConfig::default());
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    write_frame(&mut stream, br#"{"after":"kill-9"}"#)
        .await
        .unwrap();
    let got = recv_event(&mut server.events).await;
    assert_eq!(got, br#"{"after":"kill-9"}"#.to_vec());

    drop(stream);
    server.shutdown.send(true).unwrap();
    server.task.await.unwrap();
    drop(guard);
    assert!(!pid_path.exists());
}

#[tokio::test]
async fn graceful_shutdown_flushes_in_flight_and_unlinks_socket() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let mut server = start_server(&path, ServerConfig::default());

    let mut stream = UnixStream::connect(&path).await.unwrap();
    // First frame proves the connection is accepted and its handler live.
    write_frame(&mut stream, br#"{"n":1}"#).await.unwrap();
    let got = recv_event(&mut server.events).await;
    assert_eq!(got, br#"{"n":1}"#.to_vec());

    // Second frame is in flight when shutdown triggers.
    write_frame(&mut stream, br#"{"in":"flight"}"#)
        .await
        .unwrap();
    drop(stream);
    server.shutdown.send(true).unwrap();

    let got = recv_event(&mut server.events).await;
    assert_eq!(got, br#"{"in":"flight"}"#.to_vec());

    server.task.await.unwrap();
    assert!(!path.exists(), "socket file must be unlinked on shutdown");
}

#[tokio::test]
async fn max_connections_excess_client_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let config = ServerConfig {
        max_connections: 1,
        ..ServerConfig::default()
    };
    let mut server = start_server(&path, config);

    // First client occupies the single slot.
    let mut first = UnixStream::connect(&path).await.unwrap();
    write_frame(&mut first, br#"{"n":1}"#).await.unwrap();
    let got = recv_event(&mut server.events).await;
    assert_eq!(got, br#"{"n":1}"#.to_vec());

    // Second client is closed immediately.
    let mut second = UnixStream::connect(&path).await.unwrap();
    let mut buf = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(5), second.read(&mut buf))
        .await
        .expect("excess connection must be closed promptly")
        .unwrap_or(0);
    assert_eq!(n, 0);

    // Releasing the first slot lets new clients in.
    drop(first);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut third = UnixStream::connect(&path).await.unwrap();
    write_frame(&mut third, br#"{"n":3}"#).await.unwrap();
    let got = recv_event(&mut server.events).await;
    assert_eq!(got, br#"{"n":3}"#.to_vec());

    drop(third);
    server.shutdown.send(true).unwrap();
    server.task.await.unwrap();
}

#[tokio::test]
async fn idle_connection_is_closed_after_read_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let config = ServerConfig {
        read_timeout: Duration::from_millis(100),
        ..ServerConfig::default()
    };
    let server = start_server(&path, config);

    let mut idle = UnixStream::connect(&path).await.unwrap();
    let mut buf = [0u8; 1];
    let n = tokio::time::timeout(Duration::from_secs(5), idle.read(&mut buf))
        .await
        .expect("idle connection must be closed by read timeout")
        .unwrap_or(0);
    assert_eq!(n, 0);

    server.shutdown.send(true).unwrap();
    server.task.await.unwrap();
}

#[tokio::test]
async fn startup_completes_within_300ms() {
    // SPEC §16: startup ≤ 300 ms. Path resolution + PID acquisition +
    // bind is the daemon's critical startup path.
    let dir = tempfile::tempdir().unwrap();
    let paths = DaemonPaths::resolve_with_env(Some(dir.path().to_str().unwrap()), None);

    let started = Instant::now();
    paths.ensure_parent_dirs().unwrap();
    let _pid = PidFile::acquire(&paths.pid_path).unwrap();
    let _server = SocketServer::bind(&paths.socket_path, ServerConfig::default()).unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(300),
        "startup took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn frame_helpers_round_trip_over_real_socket() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_path(&dir);
    let listener = tokio::net::UnixListener::bind(&path).unwrap();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_frame(&mut stream).await.unwrap().unwrap()
    });

    let mut client = UnixStream::connect(&path).await.unwrap();
    write_frame(&mut client, br#"{"transport":"uds"}"#)
        .await
        .unwrap();

    let got = server.await.unwrap();
    assert_eq!(got, br#"{"transport":"uds"}"#.to_vec());
}
