//! Unit tests for the lean compression passes (#62).
//!
//! Included from `compress.rs` via `#[path]`, so `super` is the `compress` module. These
//! tests exercise the decoration pass directly. The table-pass tests live in
//! `tables_tests.rs`, and the end-to-end tier properties live in `tier/mod_tests.rs`.

use super::*;

#[test]
fn strip_decoration_drops_horizontal_rules() {
    let md = "Line one.\n\n---\n\nLine two.\n***\nLine three.";
    let out = strip_decoration(md);
    assert!(out.contains("Line one."));
    assert!(out.contains("Line two."));
    assert!(out.contains("Line three."));
    assert!(!out.contains("---"), "a horizontal rule survived: {out}");
    assert!(!out.contains("***"), "a horizontal rule survived: {out}");
}

#[test]
fn strip_decoration_keeps_a_setext_free_heading_and_content() {
    // A `-` bullet is not a horizontal rule, so a list item survives. Only a line of
    // three or more identical rule characters is dropped.
    let md = "# Title\n\n- a real bullet\n\n___";
    let out = strip_decoration(md);
    assert!(out.contains("# Title"));
    assert!(
        out.contains("- a real bullet"),
        "a bullet was dropped: {out}"
    );
    assert!(!out.contains("___"), "a horizontal rule survived: {out}");
}

#[test]
fn strip_decoration_keeps_the_frontmatter_fences() {
    // The leading `---` frontmatter fences precede the first heading, so they are kept. A
    // body horizontal rule after the first heading is dropped.
    let md = "---\nname: demo\ndescription: a demo\n---\n\n# Title\n\nBody.\n\n---\n\nMore body.";
    let out = strip_decoration(md);
    let fence_count = out.lines().filter(|l| l.trim() == "---").count();
    assert_eq!(fence_count, 2, "frontmatter fences not preserved: {out}");
    assert!(out.contains("# Title"));
    assert!(out.contains("More body."));
}

#[test]
fn strip_decoration_trims_trailing_whitespace() {
    let md = "text with trailing spaces   \nnext line\t";
    let out = strip_decoration(md);
    assert!(!out.contains("spaces   "), "trailing space kept: {out:?}");
    assert!(out.contains("text with trailing spaces"));
}
