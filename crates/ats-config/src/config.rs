use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub states: StatesConfig,
    #[serde(default)]
    pub rendering: RenderingConfig,
    #[serde(default)]
    pub renderers: RenderersConfig,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default, skip_serializing_if = "TtsConfig::is_empty")]
    pub tts: TtsConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

fn default_version() -> u32 {
    1
}

impl Config {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Some(start) = &self.renderers.notifications.quiet_hours.start {
            if let Some(end) = &self.renderers.notifications.quiet_hours.end {
                if start >= end {
                    errors.push(format!(
                        "quiet_hours.start ({start}) must be before quiet_hours.end ({end})"
                    ));
                }
            }
        }

        if let Some(ref socket_path) = self.daemon.socket_path {
            if socket_path.is_empty() {
                errors.push("daemon.socket_path must not be empty".to_string());
            }
        }

        for (state, entry) in &self.states.states {
            if entry.color.as_deref() == Some("") {
                errors.push(format!(
                    "states.{state}.color must not be empty; use null to disable"
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn default_config() -> Self {
        Self {
            version: 1,
            daemon: DaemonConfig::default(),
            privacy: PrivacyConfig::default(),
            states: StatesConfig::default(),
            rendering: RenderingConfig::default(),
            renderers: RenderersConfig::default(),
            providers: ProvidersConfig::default(),
            tts: TtsConfig::default(),
            logging: LoggingConfig::default(),
            theme: Some("default".to_string()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default_config()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DaemonConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default = "default_event_retention")]
    pub event_retention: String,
    #[serde(default)]
    pub socket_path: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: LogLevel::Warn,
            event_retention: "24h".to_string(),
            socket_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> LogLevel {
    LogLevel::Warn
}

fn default_event_retention() -> String {
    "24h".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrivacyConfig {
    #[serde(default)]
    pub store_activity_labels: bool,
    #[serde(default)]
    pub store_workspace_paths: bool,
    #[serde(default = "default_true")]
    pub redact_home_directory: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            store_activity_labels: false,
            store_workspace_paths: false,
            redact_home_directory: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatesConfig {
    #[serde(default, flatten)]
    pub states: HashMap<String, StateOverride>,
}

impl Default for StatesConfig {
    fn default() -> Self {
        let mut states = HashMap::new();
        states.insert(
            "working".to_string(),
            StateOverride {
                color: Some("#2457A6".to_string()),
                label: Some("Working".to_string()),
                symbol: Some("*".to_string()),
            },
        );
        states.insert(
            "attention".to_string(),
            StateOverride {
                color: Some("#D97706".to_string()),
                label: Some("Needs input".to_string()),
                symbol: Some("!".to_string()),
            },
        );
        states.insert(
            "risk".to_string(),
            StateOverride {
                color: Some("#B91C1C".to_string()),
                label: Some("Risk".to_string()),
                symbol: Some("!!".to_string()),
            },
        );
        states.insert(
            "result".to_string(),
            StateOverride {
                color: Some("#15803D".to_string()),
                label: Some("Completed".to_string()),
                symbol: Some("+".to_string()),
            },
        );
        states.insert(
            "error".to_string(),
            StateOverride {
                color: Some("#9333EA".to_string()),
                label: Some("Error".to_string()),
                symbol: Some("x".to_string()),
            },
        );
        states.insert(
            "unknown".to_string(),
            StateOverride {
                color: Some("#6B7280".to_string()),
                label: Some("Unknown".to_string()),
                symbol: Some("?".to_string()),
            },
        );
        Self { states }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateOverride {
    pub color: Option<String>,
    pub label: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct RenderingConfig {
    #[serde(default)]
    pub background: BackgroundConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackgroundConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_intensity")]
    pub intensity: BackgroundIntensity,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: BackgroundIntensity::Subtle,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundIntensity {
    Subtle,
    Medium,
    Strong,
}

fn default_intensity() -> BackgroundIntensity {
    BackgroundIntensity::Subtle
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct RenderersConfig {
    #[serde(default)]
    pub tmux: TmuxRendererConfig,
    #[serde(default)]
    pub iterm2: ITerm2RendererConfig,
    #[serde(default)]
    pub notifications: NotificationsRendererConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TmuxRendererConfig {
    #[serde(default = "default_renderer_enabled")]
    pub enabled: RendererMode,
    #[serde(default = "default_true")]
    pub pane_border: bool,
    #[serde(default = "default_true")]
    pub pane_title: bool,
}

impl Default for TmuxRendererConfig {
    fn default() -> Self {
        Self {
            enabled: RendererMode::Auto,
            pane_border: true,
            pane_title: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RendererMode {
    Auto,
    On,
    Off,
}

fn default_renderer_enabled() -> RendererMode {
    RendererMode::Auto
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ITerm2RendererConfig {
    #[serde(default = "default_renderer_enabled")]
    pub enabled: RendererMode,
    #[serde(default = "default_true")]
    pub badge: bool,
    #[serde(default = "default_true")]
    pub tab_title: bool,
    #[serde(default)]
    pub background: bool,
}

impl Default for ITerm2RendererConfig {
    fn default() -> Self {
        Self {
            enabled: RendererMode::Auto,
            badge: true,
            tab_title: true,
            background: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NotificationsRendererConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_notification_states")]
    pub states: Vec<String>,
    #[serde(default)]
    pub quiet_hours: QuietHoursConfig,
}

impl Default for NotificationsRendererConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            states: default_notification_states(),
            quiet_hours: QuietHoursConfig::default(),
        }
    }
}

fn default_notification_states() -> Vec<String> {
    vec![
        "attention".to_string(),
        "risk".to_string(),
        "result".to_string(),
        "error".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuietHoursConfig {
    pub start: Option<String>,
    pub end: Option<String>,
    #[serde(default)]
    pub allow: Vec<String>,
}

impl Default for QuietHoursConfig {
    fn default() -> Self {
        Self {
            start: Some("22:00".to_string()),
            end: Some("07:00".to_string()),
            allow: vec!["risk".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProvidersConfig {
    #[serde(default)]
    pub claude: ProviderConfig,
    #[serde(default)]
    pub opencode: ProviderConfig,
    #[serde(default)]
    pub codex: ProviderConfig,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            claude: ProviderConfig {
                enabled: true,
                experimental: false,
            },
            opencode: ProviderConfig {
                enabled: false,
                experimental: false,
            },
            codex: ProviderConfig {
                enabled: false,
                experimental: true,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub experimental: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct TtsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub states: Vec<String>,
}

impl TtsConfig {
    pub fn is_empty(&self) -> bool {
        !self.enabled && self.states.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: LogLevel,
    #[serde(default = "default_log_retention")]
    pub retention: String,
    #[serde(default = "default_true")]
    pub redact: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Warn,
            retention: "7d".to_string(),
            redact: true,
        }
    }
}

fn default_log_retention() -> String {
    "7d".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrip() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn version_defaults_to_1() {
        let yaml = "daemon:\n  log_level: info\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.version, 1);
    }

    #[test]
    fn all_log_levels_roundtrip() {
        for level in &["trace", "debug", "info", "warn", "error"] {
            let yaml = format!("daemon:\n  log_level: {level}\n");
            let config: Config = serde_yaml::from_str(&yaml).unwrap();
            let serialized = serde_yaml::to_string(&config).unwrap();
            let reparsed: Config = serde_yaml::from_str(&serialized).unwrap();
            assert_eq!(config, reparsed);
        }
    }

    #[test]
    fn all_background_intensities_roundtrip() {
        for intensity in &["subtle", "medium", "strong"] {
            let yaml = format!(
                "rendering:\n  background:\n    enabled: true\n    intensity: {intensity}\n"
            );
            let config: Config = serde_yaml::from_str(&yaml).unwrap();
            let serialized = serde_yaml::to_string(&config).unwrap();
            let reparsed: Config = serde_yaml::from_str(&serialized).unwrap();
            assert_eq!(config, reparsed);
        }
    }

    #[test]
    fn all_renderer_modes_roundtrip() {
        for mode in &["auto", "on", "off"] {
            let yaml = format!("renderers:\n  tmux:\n    enabled: {mode}\n");
            let config: Config = serde_yaml::from_str(&yaml).unwrap();
            let serialized = serde_yaml::to_string(&config).unwrap();
            let reparsed: Config = serde_yaml::from_str(&serialized).unwrap();
            assert_eq!(config, reparsed);
        }
    }

    #[test]
    fn tts_parse_and_ignore() {
        let yaml = "tts:\n  enabled: true\n  states:\n    - attention\n    - risk\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.tts.enabled);
        assert_eq!(config.tts.states, vec!["attention", "risk"]);
    }

    #[test]
    fn invalid_log_level_fails() {
        let yaml = "daemon:\n  log_level: invalid_level\n";
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn full_spec_example_roundtrip() {
        let spec_yaml = include_str!("../../../docs/SPEC.md");
        let config_start = spec_yaml.find("version: 1\n").unwrap();
        let config_end = spec_yaml[config_start..].find("\n```").unwrap();
        let config_yaml = &spec_yaml[config_start..config_start + config_end];
        let config: Config = serde_yaml::from_str(config_yaml).unwrap();
        let serialized = serde_yaml::to_string(&config).unwrap();
        let reparsed: Config = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(config, reparsed);
    }

    #[test]
    fn custom_state_overrides() {
        let yaml = r##"
states:
  working:
    color: "#FF0000"
    label: "Busy"
    symbol: ">>>"
"##;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.states.states.get("working").unwrap().color,
            Some("#FF0000".to_string())
        );
        assert_eq!(
            config.states.states.get("working").unwrap().label,
            Some("Busy".to_string())
        );
    }

    #[test]
    fn quiet_hours_validation_start_after_end() {
        let mut config = Config::default();
        config.renderers.notifications.quiet_hours.start = Some("23:00".to_string());
        config.renderers.notifications.quiet_hours.end = Some("01:00".to_string());
        let errors = config.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("quiet_hours")));
    }

    #[test]
    fn empty_socket_path_validation() {
        let mut config = Config::default();
        config.daemon.socket_path = Some("".to_string());
        let errors = config.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("socket_path")));
    }

    #[test]
    fn defaults_match_spec() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.daemon.log_level, LogLevel::Warn);
        assert_eq!(config.daemon.event_retention, "24h");
        assert!(config.daemon.enabled);
        assert!(!config.privacy.store_activity_labels);
        assert!(!config.privacy.store_workspace_paths);
        assert!(config.privacy.redact_home_directory);
        assert!(!config.rendering.background.enabled);
        assert_eq!(
            config.rendering.background.intensity,
            BackgroundIntensity::Subtle
        );
        assert_eq!(config.renderers.tmux.enabled, RendererMode::Auto);
        assert!(config.renderers.tmux.pane_border);
        assert!(config.renderers.tmux.pane_title);
        assert_eq!(config.renderers.iterm2.enabled, RendererMode::Auto);
        assert!(config.renderers.iterm2.badge);
        assert!(config.renderers.iterm2.tab_title);
        assert!(!config.renderers.iterm2.background);
        assert!(config.renderers.notifications.enabled);
        assert!(!config.tts.enabled);
        assert_eq!(config.logging.level, LogLevel::Warn);
        assert_eq!(config.logging.retention, "7d");
        assert!(config.logging.redact);
    }

    #[test]
    fn deny_unknown_fields_at_top_level() {
        let yaml = "version: 1\nunknown_field: true\n";
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn deny_unknown_fields_nested() {
        let yaml = "daemon:\n  enabled: true\n  unknown_field: yes\n";
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn empty_color_validation() {
        let mut config = Config::default();
        config.states.states.insert(
            "working".to_string(),
            StateOverride {
                color: Some("".to_string()),
                label: None,
                symbol: None,
            },
        );
        let errors = config.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("states.working.color")));
    }
}
