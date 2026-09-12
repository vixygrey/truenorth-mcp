//! Cockpit resources: `truenorth://state`, `truenorth://cockpit`, and
//! `truenorth://conventions`.
//!
//! The read, validation, and last-good cache logic for these resources lives in the
//! parent module's `ResourceDoc` and `ResourceCache` (see `resources/mod.rs`). The state
//! and cockpit resources validate against the observed cockpit schemas; conventions
//! serves raw markdown. This module is the design's named home for these three resources
//! (§1); it carries no separate logic today.
//!
//! Requirements: 5.1, 5.2, 5.5, 5.7. Design: Part II §5.
