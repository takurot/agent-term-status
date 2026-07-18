use std::path::PathBuf;

pub struct ConfigPaths {
    pub user_config_dir: PathBuf,
    pub user_state_dir: PathBuf,
    pub user_runtime_dir: PathBuf,
    pub user_themes_dir: PathBuf,
    pub user_config_file: PathBuf,
    pub project_config_file: Option<PathBuf>,
}

impl ConfigPaths {
    pub fn resolve(project_root: Option<&str>) -> Self {
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                home.join(".config")
            });

        let state_home = std::env::var("XDG_STATE_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                home.join(".local").join("state")
            });

        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);

        let user_config_dir = config_home.join("agent-term-status");
        let user_state_dir = state_home.join("agent-term-status");
        let user_runtime_dir = runtime_dir.join("agent-term-status");
        let user_themes_dir = user_config_dir.join("themes");
        let user_config_file = user_config_dir.join("config.yaml");

        let project_config_file = project_root.map(|root| {
            PathBuf::from(root)
                .join(".agent-term-status")
                .join("config.yaml")
        });

        Self {
            user_config_dir,
            user_state_dir,
            user_runtime_dir,
            user_themes_dir,
            user_config_file,
            project_config_file,
        }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.user_config_dir)?;
        std::fs::create_dir_all(&self.user_state_dir)?;
        std::fs::create_dir_all(&self.user_runtime_dir)?;
        Ok(())
    }

    pub fn user_config_file(&self) -> &PathBuf {
        &self.user_config_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_xdg_config_home() {
        std::env::set_var("XDG_CONFIG_HOME", "/custom/config");
        std::env::set_var("XDG_STATE_HOME", "/custom/state");
        std::env::set_var("XDG_RUNTIME_DIR", "/custom/runtime");

        let paths = ConfigPaths::resolve(Some("/my/project"));

        assert_eq!(
            paths.user_config_file,
            PathBuf::from("/custom/config/agent-term-status/config.yaml")
        );
        assert_eq!(
            paths.user_config_dir,
            PathBuf::from("/custom/config/agent-term-status")
        );
        assert_eq!(
            paths.user_state_dir,
            PathBuf::from("/custom/state/agent-term-status")
        );
        assert_eq!(
            paths.project_config_file,
            Some(PathBuf::from("/my/project/.agent-term-status/config.yaml"))
        );
    }

    #[test]
    fn no_project_root_yields_no_project_file() {
        let paths = ConfigPaths::resolve(None);
        assert!(paths.project_config_file.is_none());
    }

    #[test]
    fn empty_xdg_vars_fallback() {
        std::env::set_var("XDG_CONFIG_HOME", "");
        std::env::set_var("XDG_STATE_HOME", "");
        std::env::set_var("XDG_RUNTIME_DIR", "");
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_STATE_HOME");
        std::env::remove_var("XDG_RUNTIME_DIR");

        let paths = ConfigPaths::resolve(None);
        assert!(paths
            .user_config_file
            .to_string_lossy()
            .contains("agent-term-status"));
    }
}
