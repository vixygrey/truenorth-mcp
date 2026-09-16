//! Serde models for the cockpit YAML files: `state.yaml`, `release-plan.yaml`, and
//! `ontology.yaml`.
//!
//! The state and release-plan models preserve the document verbatim (Requirement 9.3).
//! Both hold the whole document in an ordered map, so a read then a write reproduces
//! every key and value, including `null`-valued fields and `bigpowers_version`
//! (Requirement 9.4). A named-field model with `skip_serializing_if` would drop a
//! present `null` field, so the map-backed model is the design that meets Property 3.
//! Typed access to the observed fields (design §3.1) comes through accessor methods.
//!
//! The ontology model carries `schemars` derives, because the ontology tool surfaces it
//! as a tool result schema (Requirement 4.3, task 12).
//!
//! Requirements: 9.1, 9.3, 9.4, 4.3. Design: Part II §3.1, §3.2, §8.

// These models are the cockpit data surface consumed by later tasks: the lifecycle and
// ontology tools (tasks 9, 12) and the resources layer (task 14) read and write them.
// The types are unused until those tasks land, so the module-scoped allow prevents a
// premature dead-code error under `clippy -D warnings`. Remove this allow once task 14
// wires the last consumer.
#![allow(dead_code)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

/// A lifecycle phase in the six-phase model.
///
/// The phase names serialize in kebab-case (`discover`, `design`, ...), matching the
/// tool contract enum (design §2). Legacy bigpowers phase names map onto this enum in
/// [`crate::engine::validate`] (Requirement 9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Understand the problem and current state.
    Discover,
    /// Model the domain and design the solution.
    Design,
    /// Break the work into tasks with verify commands.
    Plan,
    /// Build the change under a Red-Green-Refactor loop.
    Execute,
    /// Review and harden the change against the quality bar.
    Review,
    /// Integrate the change and advance the release.
    Integrate,
}

/// The `state.yaml` cockpit file (Requirement 9.1).
///
/// The document is held verbatim in `root`, so a read then a write preserves every key
/// and value unchanged (Requirements 9.3, 9.4). Typed accessors expose the observed
/// fields (design §3.1) without dropping unmodeled or `null`-valued keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct StateFile {
    /// The whole document, preserved verbatim.
    pub root: Mapping,
}

impl StateFile {
    /// The active branch from the `git.branch` field, when present.
    pub fn git_branch(&self) -> Option<&str> {
        self.root.get("git")?.get("branch")?.as_str()
    }

    /// The `bigpowers_version` value, when present (Requirement 9.4).
    pub fn bigpowers_version(&self) -> Option<&Value> {
        self.root.get("bigpowers_version")
    }

    /// Set a top-level field, inserting or replacing it in place.
    ///
    /// The write preserves every other field, so a mutation never drops a human edit
    /// (Requirement 9.3).
    pub fn set(&mut self, key: &str, value: Value) {
        self.root.insert(Value::String(key.to_string()), value);
    }

    /// Read a top-level field by key, when present.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.root.get(key)
    }
}

/// The `release-plan.yaml` cockpit file (Requirement 9.1).
///
/// The document is held verbatim in `root`, so a read then a write preserves every key
/// and value unchanged (Requirement 9.3). Typed accessors expose the observed fields
/// (design §3.1).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct ReleasePlanFile {
    /// The whole document, preserved verbatim.
    pub root: Mapping,
}

impl ReleasePlanFile {
    /// The `build_order` sequence, when present.
    pub fn build_order(&self) -> Option<&Vec<Value>> {
        self.root.get("build_order")?.as_sequence()
    }

    /// Append an entry to the `build_order` sequence, creating it when absent.
    ///
    /// Every other field is preserved (Requirement 9.3).
    pub fn push_build_order(&mut self, entry: Value) {
        let key = Value::String("build_order".to_string());
        match self.root.get_mut(&key).and_then(Value::as_sequence_mut) {
            Some(sequence) => sequence.push(entry),
            None => {
                self.root.insert(key, Value::Sequence(vec![entry]));
            }
        }
    }

    /// Read a top-level field by key, when present.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.root.get(key)
    }
}

/// The `specs/ontology.yaml` domain model (design §3.2).
///
/// The ontology carries entities and global constraints. The ontology gate scans code
/// against the entities' prohibited aliases (Property 1, task 6).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Ontology {
    /// The schema version, for example `"1"`.
    pub version: String,
    /// The domain name.
    pub domain: String,
    /// The last-updated timestamp in ISO-8601.
    pub last_updated: String,
    /// The domain entities.
    pub entities: Vec<Entity>,
    /// The global constraints.
    pub constraints: Vec<Constraint>,
}

impl Ontology {
    /// The empty stub ontology: version `1`, an empty domain, no entities, no constraints.
    ///
    /// The resource seeds this on first read, and the generate tool overwrites only a file
    /// that matches it (Requirement 4.6, 4.9). One definition, so the seed and the
    /// stub check cannot drift.
    pub fn empty_stub() -> Self {
        Self {
            version: "1".to_string(),
            domain: String::new(),
            last_updated: String::new(),
            entities: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Whether this ontology is the empty stub (Requirement 4.6).
    ///
    /// The check ignores `version` and `last_updated`, because a real ontology is defined
    /// by its domain, entities, or constraints. An empty domain with no entities and no
    /// constraints is the not-yet-defined stub, whatever its timestamp.
    pub fn is_empty_stub(&self) -> bool {
        self.domain.is_empty() && self.entities.is_empty() && self.constraints.is_empty()
    }
}

/// A domain entity in the ontology (design §3.2).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Entity {
    /// The entity name.
    pub name: String,
    /// A one-line description.
    pub description: String,
    /// The primary key field name.
    pub primary_key: String,
    /// The invariants that always hold for the entity.
    pub invariants: Vec<String>,
    /// The allowed states.
    pub states: Vec<String>,
    /// The allowed transitions, keyed by state, valued by allowed next states.
    pub transitions: BTreeMap<String, Vec<String>>,
    /// The forbidden synonyms that signal lexical drift.
    pub prohibited_aliases: Vec<String>,
}

/// A global constraint in the ontology (design §3.2).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Constraint {
    /// The constraint id, for example `"C-01"`.
    pub id: String,
    /// The rule text.
    pub rule: String,
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `spec`.
#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
