//! TrueNorth-MCP: an active, protocol-first MCP execution runtime (library crate).
//!
//! This library holds the whole runtime: the config resolution, the tools, the resources,
//! the server assembly, and the engine. The `truenorth-mcp` binary (`src/main.rs`) is a
//! thin entrypoint over this library. A second binary, `jev-bench` (`src/bin/jev_bench.rs`),
//! reuses the same library to run the Jev benchmark, so both binaries share one compilation
//! of the engine modules rather than duplicating them.
//!
//! The crate name is `truenorth-mcp`, so a binary imports this library as `truenorth_mcp`.
//! The module tree matches the design (§1). The modules stay `pub` so a binary target can
//! reach the server, the config resolver, and the engine.

// Dead code is a defect, not a warning (styleguide "No dead code"). A genuinely
// test-facing public helper carries a narrow `#[cfg_attr(not(test), allow(dead_code))]`
// with a reason; a blanket module-scoped allow is not permitted.
#![deny(dead_code)]

pub mod config;
pub mod engine;
pub mod resources;
pub mod server;
pub mod tools;

#[cfg(test)]
mod integration_tests;
