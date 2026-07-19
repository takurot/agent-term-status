//! # `ats-daemon` — local event broker daemon (SPEC §5.3)
//!
//! This crate hosts the Unix Domain Socket server that receives
//! length-prefixed JSON events from the `ats` CLI hook path and the
//! event broker (I-14) that validates, deduplicates, order-corrects,
//! and dispatches them to the state/rendering engines.
//!
//! Privacy invariant (SPEC §14.2): errors and rejections carry metadata
//! only (byte counts, never payload contents).

pub mod broker;
pub mod framing;
pub mod logging;
pub mod paths;
pub mod pidfile;
pub mod server;

pub use broker::{Broker, BrokerConfig, BrokerError};
pub use framing::{read_frame, write_frame, FrameError, MAX_FRAME_BYTES};
pub use logging::init_logging;
pub use paths::DaemonPaths;
pub use pidfile::{PidFile, PidFileError};
pub use server::{ServerConfig, SocketServer};
