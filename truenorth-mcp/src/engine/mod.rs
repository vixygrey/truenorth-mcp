//! Engine layer: YAML spec parsing and validation, gate execution, tier transforms,
//! the file watcher, git context, and ontology scanning.
//!
//! Each submodule is a skeleton stub for the crate scaffold (task 1). The real logic
//! lands in later tasks, so the module tree matches the design (§1) up front.

pub mod cockpit;
pub mod gate_runner;
pub mod git;
pub mod graph;
pub mod ontology_scan;
pub mod phase;
pub mod skill;
pub mod skill_parser;
pub mod skill_validate;
pub mod spec;
pub mod tier;
pub mod validate;
pub mod watcher;
