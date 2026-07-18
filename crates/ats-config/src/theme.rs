use ats_core::AgentState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::bundled_themes;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub name: String,
    #[serde(default)]
    pub states: HashMap<String, ThemeEntry>,
}

impl Theme {
    pub fn resolve(&self, state: AgentState) -> Option<ThemeEntry> {
        let key = agent_state_to_theme_key(state);
        self.states.get(key).cloned()
    }

    pub fn bundle_names() -> Vec<&'static str> {
        bundled_themes::BUNDLED_THEMES
            .iter()
            .map(|(name, _)| *name)
            .collect()
    }

    pub fn load_bundled(name: &str) -> Result<Theme, ThemeError> {
        let yaml = bundled_themes::BUNDLED_THEMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, y)| y)
            .ok_or_else(|| ThemeError::NotFound {
                name: name.to_string(),
                available: Self::bundle_names().iter().map(|s| s.to_string()).collect(),
            })?;
        let theme: Theme = serde_yaml::from_str(yaml).map_err(|e| ThemeError::Parse {
            name: name.to_string(),
            detail: e.to_string(),
        })?;
        Ok(theme)
    }

    pub fn load_from_path(path: &Path) -> Result<Theme, ThemeError> {
        let content = std::fs::read_to_string(path).map_err(|e| ThemeError::Io {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        let theme: Theme = serde_yaml::from_str(&content).map_err(|e| ThemeError::Parse {
            name: path.display().to_string(),
            detail: e.to_string(),
        })?;
        Ok(theme)
    }

    pub fn validate(&self) -> Result<(), ThemeError> {
        let required: Vec<AgentState> = vec![
            AgentState::Idle,
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ];

        for state in &required {
            let key = agent_state_to_theme_key(*state);
            let entry = self.states.get(key).ok_or_else(|| {
                ThemeError::Validation(format!(
                    "theme '{}' is missing state '{}'",
                    self.name,
                    agent_state_to_theme_key(*state)
                ))
            })?;
            let rep_count = [
                entry.color.is_some(),
                !entry.label.is_empty(),
                !entry.symbol.is_empty(),
            ]
            .iter()
            .filter(|&&x| x)
            .count();
            if rep_count < 2 {
                return Err(ThemeError::Validation(format!(
                    "theme '{}' state '{}' has fewer than 2 representation types",
                    self.name,
                    agent_state_to_theme_key(*state)
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeEntry {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub notification: bool,
}

fn agent_state_to_theme_key(state: AgentState) -> &'static str {
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

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("theme not found: {name}. Available: {available:?}")]
    NotFound {
        name: String,
        available: Vec<String>,
    },
    #[error("failed to parse theme '{name}': {detail}")]
    Parse { name: String, detail: String },
    #[error("failed to read theme file '{path}': {detail}")]
    Io { path: String, detail: String },
    #[error("theme validation error: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_five_bundled_themes_load() {
        let bundles = Theme::bundle_names();
        assert_eq!(bundles.len(), 5);
        for name in bundles {
            let theme = Theme::load_bundled(name).unwrap();
            assert_eq!(theme.name, name);
        }
    }

    #[test]
    fn all_bundled_themes_validate() {
        for name in Theme::bundle_names() {
            let theme = Theme::load_bundled(name).unwrap();
            theme.validate().unwrap_or_else(|e| {
                panic!("theme '{name}' failed validation: {e}");
            });
        }
    }

    #[test]
    fn theme_resolves_every_state() {
        for name in Theme::bundle_names() {
            let theme = Theme::load_bundled(name).unwrap();
            let states = [
                AgentState::Idle,
                AgentState::Working,
                AgentState::Attention,
                AgentState::Risk,
                AgentState::Result,
                AgentState::Error,
                AgentState::Unknown,
            ];
            for state in &states {
                let entry = theme.resolve(*state);
                assert!(entry.is_some(), "theme '{name}' missing state {state:?}");
            }
        }
    }

    #[test]
    fn theme_entry_two_representations() {
        let theme = Theme::load_bundled("default").unwrap();
        let entry = theme.resolve(AgentState::Working).unwrap();
        let rep_count = [
            entry.color.is_some(),
            !entry.label.is_empty(),
            !entry.symbol.is_empty(),
        ]
        .iter()
        .filter(|&&x| x)
        .count();
        assert!(rep_count >= 2, "Working has {rep_count} reps, need >= 2");
    }

    #[test]
    fn bundled_theme_not_found() {
        let err = Theme::load_bundled("nonexistent").unwrap_err();
        assert!(matches!(err, ThemeError::NotFound { .. }));
    }

    #[test]
    fn theme_roundtrip() {
        let theme = Theme::load_bundled("default").unwrap();
        let yaml = serde_yaml::to_string(&theme).unwrap();
        let parsed: Theme = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(theme, parsed);
    }
}
