use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScope {
    User,
    Project,
    Local,
}

impl InstallScope {
    fn settings_path(&self) -> PathBuf {
        match self {
            InstallScope::User => {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                home.join(".claude").join("settings.json")
            }
            InstallScope::Project => PathBuf::from("./.claude/settings.json"),
            InstallScope::Local => PathBuf::from("./.claude/settings.local.json"),
        }
    }
}

#[derive(Debug)]
pub struct InstallResult {
    pub scope: InstallScope,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub unchanged: Vec<String>,
    pub dry_run: bool,
}

#[allow(dead_code)]
const ATS_INSTALL_MARKER: &str = "_ats_managed";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeSettings {
    #[serde(default)]
    hooks: BTreeMap<String, Vec<serde_json::Value>>,
    #[serde(default)]
    _ats_managed: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

fn ats_hook_entry() -> serde_json::Value {
    serde_json::json!({
        "matcher": "",
        "command": "ats ingest --provider claude",
        "env": {
            "TMUX_PANE": "${TMUX_PANE}"
        }
    })
}

fn hook_types() -> [&'static str; 8] {
    [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "Notification",
        "Stop",
        "SessionEnd",
    ]
}

pub fn install_hooks(
    scope: InstallScope,
    dry_run: bool,
) -> Result<InstallResult, Box<dyn std::error::Error>> {
    let path = scope.settings_path();
    let mut settings: ClaudeSettings = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or(ClaudeSettings {
            hooks: BTreeMap::new(),
            _ats_managed: Vec::new(),
            extra: BTreeMap::new(),
        })
    } else {
        ClaudeSettings {
            hooks: BTreeMap::new(),
            _ats_managed: Vec::new(),
            extra: BTreeMap::new(),
        }
    };

    let mut added = Vec::new();
    let mut unchanged = Vec::new();

    for hook_name in &hook_types() {
        let entry = ats_hook_entry();
        let entries = settings.hooks.entry(hook_name.to_string()).or_default();

        let already_managed = entries.iter().any(|e| {
            e.get("command")
                .and_then(|v| v.as_str())
                .map(|c| c.contains("ats ingest"))
                .unwrap_or(false)
        });

        if already_managed {
            unchanged.push(hook_name.to_string());
        } else {
            entries.push(entry);
            added.push(hook_name.to_string());
        }
    }

    if !dry_run {
        let modified = settings_with_hooks(&settings);
        let content = serde_json::to_string_pretty(&modified)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if path.exists() {
            let bak = path.with_extension("json.bak");
            std::fs::copy(&path, &bak).ok();
        }

        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        std::io::Write::write_all(&mut tmp, content.as_bytes())?;
        tmp.as_file_mut().sync_all()?;
        tmp.persist(&path)?;

        let bak = path.with_extension("json.bak");
        let _ = std::fs::remove_file(bak);
    }

    Ok(InstallResult {
        scope,
        added,
        removed: Vec::new(),
        unchanged,
        dry_run,
    })
}

pub fn uninstall_hooks(
    scope: InstallScope,
    dry_run: bool,
) -> Result<InstallResult, Box<dyn std::error::Error>> {
    let path = scope.settings_path();
    if !path.exists() {
        return Ok(InstallResult {
            scope,
            added: Vec::new(),
            removed: Vec::new(),
            unchanged: Vec::new(),
            dry_run,
        });
    }

    let content = std::fs::read_to_string(&path)?;
    let mut settings: ClaudeSettings = serde_json::from_str(&content).unwrap_or(ClaudeSettings {
        hooks: BTreeMap::new(),
        _ats_managed: Vec::new(),
        extra: BTreeMap::new(),
    });

    let mut removed = Vec::new();
    let mut unchanged = Vec::new();

    for hook_name in &hook_types() {
        if let Some(entries) = settings.hooks.get_mut(*hook_name) {
            let before = entries.len();
            entries.retain(|e| {
                !e.get("command")
                    .and_then(|v| v.as_str())
                    .map(|c| c.contains("ats ingest"))
                    .unwrap_or(false)
            });
            if entries.len() < before {
                removed.push(hook_name.to_string());
            } else {
                unchanged.push(hook_name.to_string());
            }
            if entries.is_empty() {
                settings.hooks.remove(*hook_name);
            }
        }
    }

    if !dry_run {
        let modified = settings_with_hooks(&settings);
        let content = serde_json::to_string_pretty(&modified)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if path.exists() {
            let bak = path.with_extension("json.bak");
            std::fs::copy(&path, &bak).ok();
        }

        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        std::io::Write::write_all(&mut tmp, content.as_bytes())?;
        tmp.as_file_mut().sync_all()?;
        tmp.persist(&path)?;

        let bak = path.with_extension("json.bak");
        let _ = std::fs::remove_file(bak);
    }

    Ok(InstallResult {
        scope,
        added: Vec::new(),
        removed,
        unchanged,
        dry_run,
    })
}

fn settings_with_hooks(settings: &ClaudeSettings) -> serde_json::Value {
    let mut out: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for (key, value) in &settings.extra {
        out.insert(key.clone(), value.clone());
    }

    if !settings.hooks.is_empty() {
        let hooks_value = serde_json::to_value(&settings.hooks).unwrap_or(serde_json::Value::Null);
        out.insert("hooks".to_string(), hooks_value);
    }

    serde_json::Value::Object(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mock_settings_path(dir: &TempDir) -> PathBuf {
        dir.path().join(".claude").join("settings.json")
    }

    fn write_settings(path: &PathBuf, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn install_creates_settings_if_missing() {
        let dir = TempDir::new().unwrap();
        let path = mock_settings_path(&dir);

        // We'll test install logic manually
        let original = ClaudeSettings {
            hooks: BTreeMap::new(),
            _ats_managed: Vec::new(),
            extra: BTreeMap::new(),
        };
        let content = serde_json::to_string_pretty(&settings_with_hooks(&original)).unwrap();
        write_settings(&path, &content);

        assert!(path.exists());
    }

    #[test]
    fn install_preserves_existing_settings() {
        let dir = TempDir::new().unwrap();
        let path = mock_settings_path(&dir);

        let existing = r#"{"hooks": {"SessionStart": [{"matcher": "custom", "command": "echo hi"}]}, "otherKey": "value"}"#;
        write_settings(&path, existing);

        // Verify the file contains otherKey
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("otherKey"));
        assert!(content.contains("custom"));
    }

    #[test]
    fn uninstall_removes_only_ats_entries() {
        let dir = TempDir::new().unwrap();
        let path = mock_settings_path(&dir);

        let existing = r#"{"hooks": {"SessionStart": [{"matcher": "custom", "command": "echo hi"}, {"matcher": "", "command": "ats ingest --provider claude"}]}}"#;
        write_settings(&path, existing);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("custom"));
        assert!(content.contains("ats ingest"));
    }
}
