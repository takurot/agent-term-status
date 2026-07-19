//! `ats-daemon` binary entry point (I-13, I-14).
//!
//! Startup sequence: resolve paths → init logging → acquire PID file
//! → bind socket (`0600`) → start broker → serve until SIGTERM/SIGINT
//! → graceful drain → unlink socket and PID file.

use std::process::ExitCode;

use ats_config::theme::Theme;
use ats_daemon::{
    init_logging, Broker, BrokerConfig, DaemonPaths, PidFile, ServerConfig, SocketServer,
};
use ats_rendering::{EngineConfig, RenderingEngine};
use ats_state_engine::StateEngine;
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

    let config = ats_config::Config::default_config();
    if let Err(e) = init_logging(&paths.logs_dir, &config) {
        eprintln!("ats-daemon: failed to init logging: {e}");
    }

    let _pid_guard = PidFile::acquire(&paths.pid_path)?;
    let server = SocketServer::bind(&paths.socket_path, ServerConfig::default())?;

    let (event_tx, event_rx) = mpsc::channel::<Vec<u8>>(EVENT_CHANNEL_CAPACITY);
    let (shutdown_tx, shutdown_rx_server) = watch::channel(false);
    let shutdown_rx_broker = shutdown_tx.subscribe();

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    tokio::spawn(async move {
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
        let _ = shutdown_tx.send(true);
    });

    let theme = Theme::load_bundled("default")
        .unwrap_or_else(|_| Theme::load_bundled("color-safe").expect("bundled theme missing"));
    let rendering_engine = RenderingEngine::new(Some(theme), EngineConfig::default());

    let mut broker = Broker::new(
        StateEngine::new(),
        Some(rendering_engine),
        BrokerConfig::default(),
    );
    let broker_handle = tokio::spawn(async move {
        broker.run(event_rx, shutdown_rx_broker).await;
    });

    server.run(event_tx, shutdown_rx_server).await;
    broker_handle.await?;
    Ok(())
}
