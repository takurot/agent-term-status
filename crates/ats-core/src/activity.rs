use serde::{Deserialize, Deserializer, Serialize};

/// Activity attached to a state, e.g. `WORKING · Running tests` (SPEC §6.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    /// Coarse category; never a reason to add new state colors.
    #[serde(default)]
    pub category: ActivityCategory,
    /// Short sanitized label (SPEC §6.3.2).
    #[serde(default)]
    pub label: Option<ActivityLabel>,
    /// Provider tool name, e.g. `Bash`.
    #[serde(default)]
    pub tool_name: Option<String>,
    /// Risk classification result (SPEC §13).
    #[serde(default)]
    pub risk: Option<RiskLevel>,
}

/// Activity category taxonomy (SPEC §6.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCategory {
    Thinking,
    Reading,
    Searching,
    Editing,
    Shell,
    Testing,
    Network,
    VersionControl,
    Deployment,
    #[default]
    Unknown,
}

/// Risk level of a pending operation (SPEC §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
    #[default]
    Unknown,
}

/// Display label sanitized per SPEC §6.3.2 / §14.2.
///
/// Construction always strips Unicode `Cc` control characters (C0, DEL, C1)
/// and caps the result at [`ActivityLabel::MAX_CHARS`] characters. Lone
/// surrogates cannot occur in a Rust `str`; `serde_json` rejects them during
/// parsing. Deserialization re-applies sanitization so untrusted provider
/// JSON can never smuggle control sequences into renderers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ActivityLabel(String);

impl ActivityLabel {
    /// Maximum label length in characters (SPEC §6.3.2).
    pub const MAX_CHARS: usize = 40;

    /// Builds a label from an untrusted string, sanitizing it.
    pub fn new(raw: &str) -> Self {
        let cleaned: String = raw
            .chars()
            .filter(|c| !c.is_control())
            .take(Self::MAX_CHARS)
            .collect();
        Self(cleaned)
    }

    /// Builds a label from a path, keeping only the basename (SPEC §6.1.2).
    pub fn from_path(path: &str) -> Self {
        let basename = std::path::Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self::new(&basename)
    }

    /// Returns the sanitized label text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ActivityLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ActivityLabel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::new(&raw))
    }
}
