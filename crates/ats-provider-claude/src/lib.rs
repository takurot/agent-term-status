pub mod adapter;
mod install;
mod risk;

pub use adapter::ClaudeAdapter;
pub use install::{install_hooks, uninstall_hooks, InstallResult, InstallScope};
pub use risk::RiskClassifier;
