//! Socket and PID file path resolution (SPEC §5.3.1).
//!
//! Preferred location is `$XDG_RUNTIME_DIR`; the fallback is
//! `~/.local/state/agent-term-status/` for systems (like macOS) where
//! `XDG_RUNTIME_DIR` is not set.

use std::io;
use std::path::{Path, PathBuf};

/// Resolved daemon runtime paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonPaths {
    /// Unix domain socket path.
    pub socket_path: PathBuf,
    /// PID file path.
    pub pid_path: PathBuf,
}

impl DaemonPaths {
    /// Resolves paths from the process environment.
    pub fn resolve() -> Self {
        Self::resolve_with_env(
            std::env::var("XDG_RUNTIME_DIR").ok().as_deref(),
            dirs::home_dir().as_deref(),
        )
    }

    /// Resolves paths from explicit environment values (testable without
    /// mutating process env; see CLAUDE.md §11).
    pub fn resolve_with_env(runtime_dir: Option<&str>, home: Option<&Path>) -> Self {
        match runtime_dir.filter(|s| !s.is_empty()) {
            Some(dir) => {
                let base = PathBuf::from(dir);
                Self {
                    socket_path: base.join("agent-term-status.sock"),
                    pid_path: base.join("agent-term-status.pid"),
                }
            }
            None => {
                let base = home
                    .unwrap_or_else(|| Path::new("."))
                    .join(".local")
                    .join("state")
                    .join("agent-term-status");
                Self {
                    socket_path: base.join("status.sock"),
                    pid_path: base.join("status.pid"),
                }
            }
        }
    }

    /// Creates missing parent directories with user-only permissions
    /// (`0700`). Pre-existing directories (e.g. `$XDG_RUNTIME_DIR` itself)
    /// are left untouched.
    pub fn ensure_parent_dirs(&self) -> io::Result<()> {
        for path in [&self.socket_path, &self.pid_path] {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_dir_takes_priority() {
        let paths =
            DaemonPaths::resolve_with_env(Some("/run/user/501"), Some(Path::new("/Users/me")));
        assert_eq!(
            paths.socket_path,
            PathBuf::from("/run/user/501/agent-term-status.sock")
        );
        assert_eq!(
            paths.pid_path,
            PathBuf::from("/run/user/501/agent-term-status.pid")
        );
    }

    #[test]
    fn missing_runtime_dir_falls_back_to_state_dir() {
        let paths = DaemonPaths::resolve_with_env(None, Some(Path::new("/Users/me")));
        assert_eq!(
            paths.socket_path,
            PathBuf::from("/Users/me/.local/state/agent-term-status/status.sock")
        );
        assert_eq!(
            paths.pid_path,
            PathBuf::from("/Users/me/.local/state/agent-term-status/status.pid")
        );
    }

    #[test]
    fn empty_runtime_dir_is_treated_as_unset() {
        let paths = DaemonPaths::resolve_with_env(Some(""), Some(Path::new("/Users/me")));
        assert!(paths
            .socket_path
            .starts_with("/Users/me/.local/state/agent-term-status"));
    }

    #[test]
    fn ensure_parent_dirs_creates_fallback_dir_with_0700() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = DaemonPaths::resolve_with_env(None, Some(tmp.path()));

        paths.ensure_parent_dirs().unwrap();

        let parent = paths.socket_path.parent().unwrap();
        assert!(parent.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(parent).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[test]
    fn ensure_parent_dirs_leaves_existing_dir_permissions_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = DaemonPaths::resolve_with_env(Some(tmp.path().to_str().unwrap()), None);

        #[cfg(unix)]
        let before = {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(tmp.path()).unwrap().permissions().mode()
        };

        paths.ensure_parent_dirs().unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let after = std::fs::metadata(tmp.path()).unwrap().permissions().mode();
            assert_eq!(before, after);
        }
    }
}
