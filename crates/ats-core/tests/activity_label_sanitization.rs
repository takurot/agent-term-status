use ats_core::ActivityLabel;

#[test]
fn strips_c0_control_characters_including_escape() {
    let label = ActivityLabel::new("Run\x1b[31mning\x07 te\nsts\t");
    assert_eq!(label.as_str(), "Run[31mning tests");
}

#[test]
fn strips_del_and_c1_control_characters() {
    let raw = "safe\u{7f}\u{80}\u{9b}text";
    let label = ActivityLabel::new(raw);
    assert_eq!(label.as_str(), "safetext");
}

#[test]
fn caps_length_at_40_chars() {
    let raw = "a".repeat(41);
    let label = ActivityLabel::new(&raw);
    assert_eq!(label.as_str().chars().count(), 40);
}

#[test]
fn cap_applies_after_control_char_removal() {
    let raw = format!("{}{}", "\x01".repeat(10), "b".repeat(45));
    let label = ActivityLabel::new(&raw);
    assert_eq!(label.as_str(), "b".repeat(40));
}

#[test]
fn multibyte_chars_counted_as_chars_not_bytes() {
    let raw = "テ".repeat(41);
    let label = ActivityLabel::new(&raw);
    assert_eq!(label.as_str().chars().count(), 40);
}

#[test]
fn from_path_keeps_basename_only() {
    let label = ActivityLabel::from_path("/Users/alice/secret-project/src/main.rs");
    assert_eq!(label.as_str(), "main.rs");
}

#[test]
fn from_path_handles_trailing_slash_and_root() {
    assert_eq!(ActivityLabel::from_path("/tmp/dir/").as_str(), "dir");
    assert_eq!(ActivityLabel::from_path("/").as_str(), "");
}

#[test]
fn deserialization_sanitizes_untrusted_input() {
    let label: ActivityLabel =
        serde_json::from_str("\"evil\\u001b]0;pwned\\u0007label\"").expect("deserialize");
    assert_eq!(label.as_str(), "evil]0;pwnedlabel");
}

#[test]
fn deserialization_enforces_length_cap() {
    let long = format!("\"{}\"", "x".repeat(100));
    let label: ActivityLabel = serde_json::from_str(&long).expect("deserialize");
    assert_eq!(label.as_str().chars().count(), 40);
}

#[test]
fn serializes_as_transparent_string() {
    let label = ActivityLabel::new("Running tests");
    let json = serde_json::to_string(&label).expect("serialize");
    assert_eq!(json, "\"Running tests\"");
}
