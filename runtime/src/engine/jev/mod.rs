//! The Jev evaluation harness: a project-owned client trait, wire types, aspect
//! modules, a named fake, and a benchmark runner for the TypeSafe Jev model.
//!
//! The harness measures whether Jev fits five product aspects: tool routing, parallel
//! rigor scoring, drift guardrails, context pruning, and self-healing decisions. It is
//! engine infrastructure, not an MCP tool. It registers nothing into the tool router, no
//! gate, and no resource (jev-integration-eval design, Overview).
//!
//! The harness sits behind two independent opt-in layers (ADR-J1). The runtime `jev`
//! flag in `.agent/config/rules.yml` gates behavior; it is off by default, so a project
//! that does not opt in builds no state and makes no call. The Cargo `jev-http` feature
//! gates the networking dependency and the real `Http_Client`; the default build and the
//! offline test suite compile no HTTP client, so `cargo test` runs the full suite against
//! the named fake with no network call.
//!
//! This module is a scaffold stub for task 1 (issue #263). The trait, the wire types, the
//! `JevError` enum, and the child modules land in later tasks (issues #264 onward), so the
//! surface matches the design up front.
//!
//! Requirements: 1.1, 2.4, 11.7, 2.7. Design: jev-integration-eval, ADR-J1, module layout.
