//! Tests for the file watcher (tasks 7.3, 4.5).
//!
//! Included from `watcher.rs` via `#[path]`, so `super` is the watcher module. The
//! coalesce test drives the pure `Debouncer` with explicit instants, so it needs no
//! filesystem and no sleeping.
//!
//! Requirements: 5.6, and the URI mapping behind 5.3; 2.6 and 2.12 for the re-point.

use super::*;
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn maps_state_file_to_state_uri() {
    assert_eq!(
        map_path_to_uri(Path::new("/repo/.agent/tasks/state.yml")),
        Some(ResourceUri::State)
    );
}

#[test]
fn maps_release_plan_and_product_to_cockpit() {
    assert_eq!(
        map_path_to_uri(Path::new("/repo/.agent/tasks/release-plan.yml")),
        Some(ResourceUri::Cockpit)
    );
    // The product path moved to .agent/product/ (Requirement 2.12).
    assert_eq!(
        map_path_to_uri(Path::new("/repo/.agent/product/scope.md")),
        Some(ResourceUri::Cockpit)
    );
}

#[test]
fn maps_ontology_and_conventions() {
    assert_eq!(
        map_path_to_uri(Path::new("/repo/.agent/ontology.yml")),
        Some(ResourceUri::Ontology)
    );
    assert_eq!(
        map_path_to_uri(Path::new("/repo/CONVENTIONS.md")),
        Some(ResourceUri::Conventions)
    );
}

#[test]
fn maps_adr_directory_to_the_adr_uri() {
    // A change under specs/adr/ maps to the ADR resource (Requirement 9.3).
    assert_eq!(
        map_path_to_uri(Path::new("/repo/specs/adr/0001-verb-noun-naming.md")),
        Some(ResourceUri::Adr)
    );
}

#[test]
fn legacy_specs_product_is_no_longer_watched() {
    // The watcher no longer maps specs/product/ to a resource (Requirement 2.12).
    assert_eq!(
        map_path_to_uri(Path::new("/repo/specs/product/SCOPE.md")),
        None
    );
}

#[test]
fn skill_and_unrelated_paths_map_to_none() {
    assert_eq!(
        map_path_to_uri(Path::new("/repo/skills/deploy/SKILL.md")),
        None
    );
    assert_eq!(map_path_to_uri(Path::new("/repo/src/main.rs")), None);
}

#[test]
fn two_edits_in_window_coalesce_into_one_notification() {
    // Requirement 5.6: two edits within the debounce window coalesce.
    let mut debouncer = Debouncer::new();
    let start = Instant::now();

    // Two edits to the same file, both inside the window.
    debouncer.record(Path::new("/repo/.agent/tasks/state.yml"), start);
    debouncer.record(
        Path::new("/repo/.agent/tasks/state.yml"),
        start + Duration::from_millis(50),
    );

    // Before the window elapses, nothing flushes.
    assert!(debouncer.poll(start + Duration::from_millis(100)).is_none());

    // After the window, exactly one report with one URI.
    let report = debouncer
        .poll(start + DEBOUNCE_WINDOW + Duration::from_millis(1))
        .expect("the window elapsed");
    assert_eq!(report, vec![ResourceUri::State]);
}

#[test]
fn distinct_files_in_window_report_each_uri_once() {
    let mut debouncer = Debouncer::new();
    let start = Instant::now();

    debouncer.record(Path::new("/repo/.agent/tasks/state.yml"), start);
    debouncer.record(Path::new("/repo/.agent/ontology.yml"), start);
    debouncer.record(
        Path::new("/repo/.agent/tasks/state.yml"),
        start + Duration::from_millis(10),
    );

    let report = debouncer
        .poll(start + DEBOUNCE_WINDOW + Duration::from_millis(1))
        .expect("the window elapsed");
    // Two distinct URIs, each once. Ordering is stable by the enum order.
    assert_eq!(report, vec![ResourceUri::State, ResourceUri::Ontology]);
}

#[test]
fn an_adr_edit_reports_the_adr_uri_within_the_window() {
    // Requirement 9.3: a change under specs/adr/ coalesces and reports the ADR resource,
    // which the watcher emits as resources/updated within 1 second. The debouncer drives
    // the same report path the running watcher uses, deterministically.
    let mut debouncer = Debouncer::new();
    let start = Instant::now();

    debouncer.record(Path::new("/repo/specs/adr/0001-verb-noun-naming.md"), start);
    assert!(debouncer.poll(start + Duration::from_millis(100)).is_none());

    let report = debouncer
        .poll(start + DEBOUNCE_WINDOW + Duration::from_millis(1))
        .expect("the window elapsed");
    assert_eq!(report, vec![ResourceUri::Adr]);
    // The window is 200 ms, well within the 1 second bound.
    assert!(DEBOUNCE_WINDOW < Duration::from_secs(1));
}

#[test]
fn poll_before_any_change_reports_nothing() {
    let mut debouncer = Debouncer::new();
    assert!(debouncer.poll(Instant::now()).is_none());
}

#[test]
fn a_new_window_opens_after_a_flush() {
    let mut debouncer = Debouncer::new();
    let start = Instant::now();

    debouncer.record(Path::new("/repo/.agent/tasks/state.yml"), start);
    let first = debouncer.poll(start + DEBOUNCE_WINDOW + Duration::from_millis(1));
    assert_eq!(first, Some(vec![ResourceUri::State]));

    // A later change opens a fresh window and flushes on its own schedule.
    let later = start + Duration::from_secs(1);
    debouncer.record(Path::new("/repo/.agent/ontology.yml"), later);
    assert!(debouncer.poll(later + Duration::from_millis(10)).is_none());
    let second = debouncer.poll(later + DEBOUNCE_WINDOW + Duration::from_millis(1));
    assert_eq!(second, Some(vec![ResourceUri::Ontology]));
}

#[test]
fn ignored_paths_do_not_open_a_window() {
    let mut debouncer = Debouncer::new();
    let start = Instant::now();
    debouncer.record(Path::new("/repo/src/main.rs"), start);
    assert!(!debouncer.has_pending());
    assert!(
        debouncer
            .poll(start + DEBOUNCE_WINDOW + Duration::from_millis(1))
            .is_none()
    );
}
