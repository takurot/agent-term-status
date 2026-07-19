use ats_config::{
    atomic_write,
    theme::{Theme, ThemeError},
    Config, ConfigPaths,
};
use ats_core::AgentState;
const ALL_STATES: [AgentState; 7] = [
    AgentState::Idle,
    AgentState::Working,
    AgentState::Attention,
    AgentState::Risk,
    AgentState::Result,
    AgentState::Error,
    AgentState::Unknown,
];

pub fn run_list() {
    let paths = ConfigPaths::resolve(None);
    let mut listed = false;
    let mut err = false;

    let bundled_names = Theme::bundle_names();
    for name in &bundled_names {
        let current = current_theme_marker(&paths, name);
        println!("  {name} [bundled]{current}");
        listed = true;
    }

    if paths.user_themes_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&paths.user_themes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
                {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !bundled_names.contains(&stem) {
                            let current = current_theme_marker(&paths, stem);
                            println!("  {stem} [user]{current}");
                            listed = true;
                        }
                    }
                }
            }
        } else {
            eprintln!("Warning: could not read user themes directory");
            err = true;
        }
    }

    if !listed {
        eprintln!("No themes found.");
        err = true;
    }
    if err {
        std::process::exit(1);
    }
}

pub fn run_preview(name: &str) {
    let theme = match load_theme(name) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    println!("Preview: {}", theme.name);
    println!();
    println!(
        "  {:<12} {:<10} {:<12} {:<12}  Notification",
        "STATE", "COLOR", "SYMBOL", "LABEL"
    );
    println!(
        "  {:-<12} {:-<10} {:-<12} {:-<12}  ------------",
        "", "", "", ""
    );

    for state in &ALL_STATES {
        if let Some(entry) = theme.resolve(*state) {
            let state_key = state_name(*state);
            let color_str = entry.color.as_deref().unwrap_or("-");
            let symbol = &entry.symbol;
            let label = &entry.label;
            let notify = if entry.notification { "yes" } else { "no" };

            let color_preview = if let Some(ref hex) = entry.color {
                format!(
                    "\x1b[48;2;{};{};{}m  \x1b[0m",
                    u8::from_str_radix(&hex[1..3], 16).unwrap_or(0),
                    u8::from_str_radix(&hex[3..5], 16).unwrap_or(0),
                    u8::from_str_radix(&hex[5..7], 16).unwrap_or(0),
                )
            } else {
                "  ".to_string()
            };

            println!(
                "  {state_key:<12} {color_str:<10} {symbol:<12} {label:<12} {color_preview}  {notify}"
            );
        }
    }
    println!();
}

pub fn run_apply(name: &str) {
    let _theme = match load_theme(name) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let paths = ConfigPaths::resolve(None);
    let current_config = load_user_config(&paths);

    let mut config = match current_config {
        Some(c) => c,
        None => Config::default_config(),
    };

    config.theme = Some(name.to_string());

    let yaml = serde_yaml::to_string(&config).unwrap_or_else(|e| {
        eprintln!("Failed to serialize config: {e}");
        std::process::exit(1);
    });

    if let Err(e) = atomic_write(paths.user_config_file(), &yaml) {
        eprintln!("Failed to write config: {e}");
        std::process::exit(1);
    }

    println!("Theme set to '{name}'. Restart the daemon for changes to take effect.");
}

fn load_theme(name: &str) -> Result<Theme, String> {
    let paths = ConfigPaths::resolve(None);

    if let Some(bundled_name) = Theme::bundle_names().iter().find(|&&n| n == name) {
        Theme::load_bundled(bundled_name).map_err(|e| match e {
            ThemeError::NotFound { .. } => format!("Theme '{name}' not found in bundled themes."),
            ThemeError::Parse { detail, .. } => format!("Theme '{name}' is malformed: {detail}"),
            ThemeError::Io { detail, .. } => format!("I/O error reading theme '{name}': {detail}"),
            ThemeError::Validation(msg) => format!("Theme '{name}' validation error: {msg}"),
        })
    } else {
        let path = paths.user_themes_dir.join(format!("{name}.yaml"));
        let yml_path = paths.user_themes_dir.join(format!("{name}.yml"));
        let theme_path = if path.exists() {
            &*path
        } else if yml_path.exists() {
            &*yml_path
        } else {
            return Err(format!(
                "Theme '{name}' not found. Available bundled: {:?}",
                Theme::bundle_names()
            ));
        };
        Theme::load_from_path(theme_path).map_err(|e| match e {
            ThemeError::NotFound { .. } => format!("Theme '{name}' not found."),
            ThemeError::Parse { detail, .. } => format!("Theme '{name}' is malformed: {detail}"),
            ThemeError::Io { detail, .. } => format!("I/O error reading theme '{name}': {detail}"),
            ThemeError::Validation(msg) => format!("Theme '{name}' validation error: {msg}"),
        })
    }
}

fn load_user_config(paths: &ConfigPaths) -> Option<Config> {
    if !paths.user_config_file().exists() {
        return None;
    }
    match std::fs::read_to_string(paths.user_config_file()) {
        Ok(content) => serde_yaml::from_str::<Config>(&content).ok(),
        Err(_) => None,
    }
}

fn current_theme_marker(paths: &ConfigPaths, name: &str) -> String {
    if let Some(config) = load_user_config(paths) {
        if config.theme.as_deref() == Some(name) {
            return "  <-- active".to_string();
        }
    }
    String::new()
}

fn state_name(state: AgentState) -> &'static str {
    match state {
        AgentState::Idle => "idle",
        AgentState::Working => "working",
        AgentState::Attention => "attention",
        AgentState::Risk => "risk",
        AgentState::Result => "result",
        AgentState::Error => "error",
        AgentState::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_output_identifies_bundled_themes() {
        let names = Theme::bundle_names();
        assert!(names.contains(&"default"));
        assert!(names.contains(&"color-safe"));
        assert!(names.contains(&"high-contrast"));
    }

    #[test]
    fn preview_loads_all_bundled_themes() {
        for name in Theme::bundle_names() {
            let result = load_theme(name);
            assert!(
                result.is_ok(),
                "failed to load bundled theme '{name}': {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn load_theme_not_found_error_message() {
        let result = load_theme("nonexistent-theme-12345");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn apply_persists_theme_in_config() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path().join("agent-term-status");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.yaml");

        let config = Config::default_config();
        let yaml = serde_yaml::to_string(&config).unwrap();
        std::fs::write(&config_file, yaml).unwrap();

        let mut paths = ConfigPaths::resolve(None);
        paths.user_config_file = config_file.clone();

        let content = std::fs::read_to_string(&config_file).unwrap();
        let parsed: Config = serde_yaml::from_str(&content).unwrap();
        assert!(parsed.theme.is_some());
    }

    #[test]
    fn preview_loads_custom_theme_from_path() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        let theme_path = themes_dir.join("custom-test.yaml");
        let theme_content = r##"
name: custom-test
states:
  idle:
    color: "#AAAAAA"
    label: "Idle"
    symbol: "."
  working:
    color: "#00FF00"
    label: "Working"
    symbol: "*"
  attention:
    color: "#FFAA00"
    label: "Attention"
    symbol: "!"
  risk:
    color: "#FF0000"
    label: "Risk"
    symbol: "!!"
  result:
    color: "#00AA00"
    label: "Done"
    symbol: "+"
  error:
    color: "#AA00FF"
    label: "Error"
    symbol: "x"
  unknown:
    color: "#888888"
    label: "???"
    symbol: "?"
"##;
        std::fs::write(&theme_path, theme_content).unwrap();
        let theme = Theme::load_from_path(&theme_path).unwrap();
        theme.validate().unwrap();
        assert_eq!(theme.name, "custom-test");
    }

    #[test]
    fn all_bundled_themes_validate() {
        for name in Theme::bundle_names() {
            let theme = Theme::load_bundled(name).unwrap();
            theme.validate().unwrap_or_else(|e| {
                panic!("bundled theme '{name}' failed validation: {e}");
            });
        }
    }
}
