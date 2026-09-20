//! Structural and smoke checks for the Jev harness (task 13).
//!
//! Included from `mod.rs` via `#[path]`, so `super` is the `jev` module. These are
//! structural guards, not behavior tests. They assert the benchmark bin carries no test,
//! the harness hardcodes no per-token price in an acceptance-tested path, and both clients
//! satisfy the `JevClient` trait (Requirement 11.10, 12.8, 2.1, 2.2, 2.3).

use std::path::PathBuf;

/// The runtime crate root, from the cargo manifest directory.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn the_benchmark_bin_carries_no_test() {
    // The benchmark is a separate bin, so `cargo test` must not run it (R11.10). A `#[test]`
    // in the bin source would make the test harness compile and run it under the feature.
    let bin = crate_root().join("src/bin/jev_bench.rs");
    let source = std::fs::read_to_string(&bin).expect("read the benchmark bin source");
    assert!(
        !source.contains("#[test]") && !source.contains("#[tokio::test]"),
        "the benchmark bin must carry no test attribute (R11.10)"
    );
}

#[test]
fn no_per_token_price_literal_in_the_library_paths() {
    // The price comes only from config; the harness hardcodes none (R12.8). Scan the aspect
    // and runner sources for a `_per_million` literal assignment outside the config module.
    // The config module holds the schema field names, not a hardcoded value, and the tests
    // and the benchmark accounting legitimately name the fields, so this scan targets the
    // aspect logic files where a stray price constant would be a defect.
    let jev_dir = crate_root().join("src/engine/jev");
    let aspect_files = [
        "drift.rs",
        "routing.rs",
        "rigor.rs",
        "pruning.rs",
        "self_heal.rs",
        "confidence.rs",
    ];
    for file in aspect_files {
        let path = jev_dir.join(file);
        let source = std::fs::read_to_string(&path).expect("read an aspect source");
        assert!(
            !source.contains("per_million"),
            "{file} must name no per-token price; the price comes only from config (R12.8)"
        );
    }
}

/// A compile-time assertion that a type satisfies [`super::JevClient`].
///
/// The function is never called. Its bound proves at compile time that `T` implements the
/// trait, so a client that stopped satisfying it would fail the build (R2.1, R2.2, R2.3).
#[allow(dead_code)]
fn assert_is_jev_client<T: super::JevClient>() {}

#[test]
fn both_clients_satisfy_the_jev_client_trait() {
    // The fake is always compiled. The bound proves it satisfies the trait. The real client
    // is behind the feature, so its assertion is feature-gated below.
    assert_is_jev_client::<super::client_fake::FakeClient>();
}

#[cfg(feature = "jev-http")]
#[test]
fn the_http_client_satisfies_the_jev_client_trait() {
    // The real client satisfies the trait under the feature build (R2.3).
    assert_is_jev_client::<super::client_http::HttpClient>();
}
