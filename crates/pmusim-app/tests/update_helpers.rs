use chrono::{DateTime, Duration, Utc};
use pmusim_app::update::{is_skipped, should_check};

fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

#[test]
fn should_check_when_no_prior_check() {
    assert!(should_check(
        None,
        ts("2026-08-13T10:00:00Z"),
        Duration::hours(6)
    ));
}

#[test]
fn should_skip_within_throttle_window() {
    assert!(!should_check(
        Some(ts("2026-08-13T08:00:00Z")),
        ts("2026-08-13T10:00:00Z"),
        Duration::hours(6)
    ));
}

#[test]
fn should_check_after_throttle_window() {
    assert!(should_check(
        Some(ts("2026-08-13T03:00:00Z")),
        ts("2026-08-13T10:00:00Z"),
        Duration::hours(6)
    ));
}

#[test]
fn skipped_when_versions_match() {
    assert!(is_skipped(Some("0.13.2"), "0.13.2"));
}

#[test]
fn not_skipped_without_a_saved_version() {
    assert!(!is_skipped(None, "0.13.2"));
}

#[test]
fn skipped_release_does_not_hide_a_newer_version() {
    assert!(!is_skipped(Some("0.13.2"), "0.13.3"));
}
