//! Ontology resource: `truenorth://ontology`.
//!
//! The read, parse, and last-good cache logic for this resource lives in the parent
//! module's `ResourceDoc` and `ResourceCache` (see `resources/mod.rs`). The ontology
//! resource parses `specs/ontology.yaml` as a YAML document and serves it as-is. This
//! module is the design's named home for the ontology resource (§1); it carries no
//! separate logic today.
//!
//! Requirements: 5.1, 5.2, 5.5, 5.7. Design: Part II §6.
