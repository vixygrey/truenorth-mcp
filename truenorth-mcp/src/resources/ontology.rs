//! Ontology resource: `truenorth://ontology`.
//!
//! The read, parse, and last-good cache logic for this resource lives in the parent
//! module's `ResourceDoc` and `ResourceCache` (see `resources/mod.rs`). The ontology
//! resource parses `.agent/ontology.yml` as a YAML document and serves it as-is. When
//! the backing file is absent, the first read creates it under `.agent/` through the
//! single write guard (Requirement 2.4). This module is the design's named home for the
//! ontology resource (§1); it carries no separate logic today.
//!
//! Requirements: 2.3, 2.4, 2.5, 5.1, 5.2, 5.5, 5.7. Design: agent-workspace-profiles §2.1.
