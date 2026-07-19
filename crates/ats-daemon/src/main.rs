//! `ats-daemon` binary entry point (I-13).
//!
//! Startup sequence: resolve paths → acquire PID file (stale detection)
//! → bind socket (`0600`) → serve until SIGTERM/SIGINT → graceful drain
//! → unlink socket and PID file.

use std::process::ExitCode;

use ats_daemon::{DaemonPaths, PidFile, ServerConfig, SocketServer};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{mpsc, watch};

const EVENT_CHANNEL_CAPACITY: usize = 1024;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ats-daemon: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let paths = DaemonPaths::resolve();
    paths.ensure_parent_dirs()?;

    let _pid_guard = PidFile::acquire(&paths.pid_path)?;
    let server = SocketServer::bind(&paths.socket_path, ServerConfig::default())?;

    let (event_tx, mut event_rx) = mpsc::channel::<Vec<u8>>(EVENT_CHANNEL_CAPACITY);
    // Placeholder consumer until the broker lands (I-14): drain and drop.
    let broker = tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ats-daemon: failed to install SIGTERM handler: {e}");
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ats-daemon: failed to install SIGINT handler: {e}");
                return;
            }
        };
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
        let _ = shutdown_tx.send(true);
    });

    server.run(event_tx, shutdown_rx).await;
    broker.await?;
    Ok(())
}
