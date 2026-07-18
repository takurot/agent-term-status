use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Generates a new time-sortable UUIDv7 event ID (SPEC §6.1.1).
pub fn new_event_id() -> Uuid {
    Uuid::now_v7()
}

/// Returns the current UTC timestamp for `NormalizedEvent.timestamp`.
pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}
