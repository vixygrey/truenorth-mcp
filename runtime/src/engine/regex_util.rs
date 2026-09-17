//! Compile-time-constant regex construction.
//!
//! The crate builds several regexes from string literals that are fixed at build time.
//! Such a pattern cannot fail to compile at runtime, so the failure branch is dead. This
//! module centralizes that single unavoidable panic behind one documented function, so no
//! other module carries an `expect` on a constant regex.
//!
//! A caller passes only a `&'static str` literal, so the input is always a build constant.
//! A malformed literal is a programmer error caught by the first test run, not a runtime
//! condition, so a panic is the correct and only sensible outcome.

use regex::Regex;

/// Compile a regex from a build-constant pattern.
///
/// # Panics
///
/// Panics when `pattern` is not a valid regex. This is a single, documented waiver of the
/// "no expect in library code" rule (styleguide "Do It Right"): the pattern is a compile
/// constant, so an invalid one is a programmer error a test catches, never a runtime
/// input. Centralizing the panic here keeps every call site free of `expect`.
pub fn compile_static(pattern: &'static str) -> Regex {
    // A `&'static str` literal is fixed at build time, so this cannot fail at runtime.
    Regex::new(pattern).unwrap_or_else(|error| {
        panic!("invalid build-constant regex `{pattern}`: {error}");
    })
}

#[cfg(test)]
mod tests {
    use super::compile_static;

    #[test]
    fn compiles_a_valid_pattern() {
        let re = compile_static(r"^e[0-9]+$");
        assert!(re.is_match("e12"));
        assert!(!re.is_match("x12"));
    }

    #[test]
    #[should_panic(expected = "invalid build-constant regex")]
    fn panics_on_an_invalid_pattern() {
        // An unbalanced group is a programmer error, so a panic is correct.
        let _ = compile_static(r"(unbalanced");
    }
}
