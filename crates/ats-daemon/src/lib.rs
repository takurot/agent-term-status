//! # `ats-daemon` — local event broker daemon (SPEC §5.3)
//!
//! This crate hosts the Unix Domain Socket server that receives
//! length-prefixed JSON events from the `ats` CLI hook path and forwards
//! them to the broker (I-14). Transport only: no event interpretation
//! happens here.
//!
//! Privacy invariant (SPEC §14.2): errors and rejections carry metadata
//! only (byte counts, never payload contents).

pub mod framing;
pub mod paths;
pub mod pidfile;
pub mod server;

pub use framing::{read_frame, write_frame, FrameError, MAX_FRAME_BYTES};
pub use paths::DaemonPaths;
pub use pidfile::{PidFile, PidFileError};
pub use server::{ServerConfig, SocketServer};
