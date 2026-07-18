use ats_core::{new_event_id, now_utc};

#[test]
fn event_ids_are_uuid_v7() {
    let id = new_event_id();
    assert_eq!(id.get_version_num(), 7);
}

#[test]
fn event_ids_are_time_sortable_and_unique() {
    let ids: Vec<_> = (0..100).map(|_| new_event_id()).collect();
    for pair in ids.windows(2) {
        assert!(pair[0] < pair[1], "UUIDv7 must be monotonically sortable");
    }
}

#[test]
fn now_utc_formats_as_rfc3339_with_z() {
    let ts = now_utc();
    let s = serde_json::to_value(ts).expect("serialize timestamp");
    let s = s.as_str().expect("string");
    assert!(s.ends_with('Z'), "must be UTC Z-suffixed: {s}");
    let parsed: chrono::DateTime<chrono::Utc> = s.parse().expect("must parse as RFC 3339");
    assert_eq!(parsed, ts);
}
