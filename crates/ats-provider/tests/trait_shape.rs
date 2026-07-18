use ats_core::{
    new_event_id, now_utc, EventType, NormalizedEvent, SessionIdentity, SCHEMA_VERSION,
};
use ats_provider::{ProviderAdapter, ProviderError, ValidationResult};
use serde_json::{json, Value};

/// Compile-only mock verifying the trait shape (I-03 DoD).
struct MockAdapter;

impl ProviderAdapter for MockAdapter {
    fn name(&self) -> &str {
        "mock"
    }

    fn version(&self) -> &str {
        "0.0.1"
    }

    fn parse(&self, input: Value) -> Result<Vec<NormalizedEvent>, ProviderError> {
        if input.get("boom").is_some() {
            return Err(ProviderError::Malformed("boom".to_string()));
        }
        Ok(vec![NormalizedEvent {
            schema_version: SCHEMA_VERSION.to_string(),
            event_id: new_event_id(),
            timestamp: now_utc(),
            provider: self.name().to_string(),
            provider_version: Some(self.version().to_string()),
            event_type: EventType::SessionStarted,
            session: self.derive_session(&input),
            activity: None,
            metadata: serde_json::Map::new(),
        }])
    }

    fn validate(&self, input: &Value) -> ValidationResult {
        if input.is_object() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid {
                reason: "expected a JSON object".to_string(),
            }
        }
    }

    fn derive_session(&self, input: &Value) -> SessionIdentity {
        SessionIdentity {
            id: input
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            ..SessionIdentity::default()
        }
    }
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn provider_adapter_is_object_safe_and_send_sync() {
    assert_send_sync::<Box<dyn ProviderAdapter>>();
    let adapter: Box<dyn ProviderAdapter> = Box::new(MockAdapter);
    assert_eq!(adapter.name(), "mock");
    assert_eq!(adapter.version(), "0.0.1");
}

#[test]
fn mock_adapter_parses_into_normalized_events() {
    let adapter = MockAdapter;
    let events = adapter
        .parse(json!({ "session_id": "s-1" }))
        .expect("parse succeeds");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].provider, "mock");
    assert_eq!(events[0].session.id, "s-1");
}

#[test]
fn validation_result_reports_invalid_input() {
    let adapter = MockAdapter;
    assert_eq!(adapter.validate(&json!({})), ValidationResult::Valid);
    let invalid = adapter.validate(&json!("not an object"));
    assert!(matches!(invalid, ValidationResult::Invalid { .. }));
    assert!(!invalid.is_valid());
    assert!(adapter.validate(&json!({})).is_valid());
}

#[test]
fn derive_session_uses_provider_session_id_with_fallback() {
    let adapter = MockAdapter;

    let session = adapter.derive_session(&json!({ "session_id": "s-99" }));
    assert_eq!(session.id, "s-99");
    assert!(session.parent_id.is_none());
    assert!(session.terminal.is_none());

    let fallback = adapter.derive_session(&json!({}));
    assert_eq!(fallback.id, "unknown", "missing fields must not panic");
}

#[test]
fn provider_error_variants_display_context() {
    let errors = [
        ProviderError::SchemaMismatch("unexpected shape".to_string()),
        ProviderError::MissingRequiredField("session_id".to_string()),
        ProviderError::Malformed("truncated".to_string()),
    ];
    for error in errors {
        let msg = error.to_string();
        assert!(!msg.is_empty());
        let _: &dyn std::error::Error = &error;
    }
    assert!(
        ProviderError::MissingRequiredField("session_id".to_string())
            .to_string()
            .contains("session_id")
    );
}
