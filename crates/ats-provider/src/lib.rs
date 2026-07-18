//! `ats-provider` — the Provider Adapter trait surface (SPEC §5.2.1).
//!
//! Providers translate tool-specific native events into
//! [`NormalizedEvent`]s. This crate holds only the trait and its error
//! types so provider implementations (e.g. `ats-provider-claude`) and the
//! daemon can evolve independently. Depends only on `ats-core`.

use ats_core::{NormalizedEvent, SessionIdentity};
use serde_json::Value;

/// Adapter converting provider-native events into normalized events.
///
/// Defined in SPEC §5.2.1. Implementations MUST follow the
/// forward-compatibility requirements of SPEC §5.2.2:
///
/// - **Ignore unknown fields** in provider input (forward-compatible).
/// - **Never crash on missing fields**: return a [`ProviderError`] or a
///   validation-error event instead of panicking.
/// - Emit `provider.schema_error` (as [`ats_core::EventType::ProviderSchemaError`])
///   for unknown/incompatible payloads rather than guessing.
/// - Record provider name and version on every emitted event.
/// - Keep a schema version so provider format changes are detectable.
pub trait ProviderAdapter: Send + Sync {
    /// Stable provider name recorded on events, e.g. `claude`.
    fn name(&self) -> &str;

    /// Provider adapter version recorded on events.
    fn version(&self) -> &str;

    /// Converts one native payload into zero or more normalized events.
    ///
    /// Must not panic on unexpected input; return a [`ProviderError`]
    /// instead so the caller can fail open (SPEC §15).
    fn parse(&self, input: Value) -> Result<Vec<NormalizedEvent>, ProviderError>;

    /// Cheap structural validation, run before [`ProviderAdapter::parse`].
    fn validate(&self, input: &Value) -> ValidationResult;

    /// Derives the session identity from a native payload, falling back to
    /// the priority order of SPEC §6.4.1 when fields are missing.
    fn derive_session(&self, input: &Value) -> SessionIdentity;
}

/// Parse failure reported by a [`ProviderAdapter`].
///
/// These errors must be treated as data, never as reasons to crash the
/// hook path (fail-open, SPEC §15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// Input shape does not match any known provider schema version.
    SchemaMismatch(String),
    /// A field required by SPEC §6.1.1 is missing from the input.
    MissingRequiredField(String),
    /// Input is not decodable (truncated, wrong types, oversized, ...).
    Malformed(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaMismatch(detail) => {
                write!(f, "input does not match provider schema: {detail}")
            }
            Self::MissingRequiredField(field) => {
                write!(f, "required field missing: {field}")
            }
            Self::Malformed(detail) => write!(f, "malformed provider input: {detail}"),
        }
    }
}

impl std::error::Error for ProviderError {}

/// Result of structural validation of a provider payload.
///
/// `Invalid` inputs are converted into `provider.schema_error` events by
/// the caller (SPEC §5.2.2); they never abort the hook path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Payload is structurally usable.
    Valid,
    /// Payload cannot be used; `reason` is safe for logs (no payload bodies).
    Invalid {
        /// Human-readable reason, free of payload contents.
        reason: String,
    },
}

impl ValidationResult {
    /// Returns `true` when the payload passed validation.
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}
