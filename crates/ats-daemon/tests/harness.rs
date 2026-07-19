mod common;

use common::{tmux_available, TmuxDriver};

#[test]
fn tmux_driver_isolation() {
    if !tmux_available() {
        return;
    }

    let driver_a = TmuxDriver::new("iso-a");
    let driver_b = TmuxDriver::new("iso-b");

    let panes_a = driver_a.pane_ids();
    let panes_b = driver_b.pane_ids();

    assert!(!panes_a.is_empty(), "driver A should have panes");
    assert!(!panes_b.is_empty(), "driver B should have panes");

    driver_a.set_pane_option(&panes_a[0], "pane-border-style", "fg=blue");
    driver_b.set_pane_option(&panes_b[0], "pane-border-style", "fg=red");

    let style_a = driver_a.show_pane_option(&panes_a[0], "pane-border-style");
    let style_b = driver_b.show_pane_option(&panes_b[0], "pane-border-style");

    assert_eq!(style_a, "pane-border-style fg=blue");
    assert_eq!(style_b, "pane-border-style fg=red");
}

#[test]
fn tmux_driver_pane_format() {
    if !tmux_available() {
        return;
    }

    let driver = TmuxDriver::new("fmt-test");
    let pane = driver.first_pane().unwrap();

    let pane_id = driver.pane_format(&pane, "#{pane_id}");
    assert_eq!(pane_id, pane);

    let pane_index = driver.pane_format(&pane, "#{pane_index}");
    assert!(!pane_index.is_empty());
}

#[test]
fn tmux_driver_pane_options() {
    if !tmux_available() {
        return;
    }

    let driver = TmuxDriver::new("opt-test");
    let pane = driver.first_pane().unwrap();

    assert!(driver.set_pane_option(&pane, "pane-border-style", "fg=red"));

    let style = driver.show_pane_option(&pane, "pane-border-style");
    assert_eq!(style, "pane-border-style fg=red");
}

#[test]
fn tmux_driver_split_pane() {
    if !tmux_available() {
        return;
    }

    let driver = TmuxDriver::new("split-t");
    let panes = driver.pane_ids();
    assert_eq!(panes.len(), 1, "should start with one pane");

    let sibling = driver.create_pane();
    assert!(sibling.is_some(), "should create sibling pane");
    assert_eq!(
        driver.pane_ids().len(),
        2,
        "should have two panes after split"
    );
}
