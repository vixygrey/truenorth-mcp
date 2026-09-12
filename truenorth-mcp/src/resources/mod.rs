//! Resources layer: serves the cockpit and ontology files as `truenorth://` MCP
//! resources.
//!
//! Four resources are served, each backed by a file on disk (Requirements 5.1, 5.2). A
//! read returns the current on-disk content, so disk is the source of truth (Requirement
//! 5.5). When a backing file fails to parse or validate, the read returns an error that
//! names the file, the layer retains the last successfully parsed content, and the other
//! resources keep serving (Requirement 5.7).
//!
//! Requirements: 5.1, 5.2, 5.5, 5.7. Design: Part II §5, §6.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::engine::validate::{validate_release_plan, validate_state};

pub mod cockpit;
pub mod ontology;

/// A served resource, identified by its `truenorth://` URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceDoc {
    /// `truenorth://state`, backed by `specs/state.yaml`.
    State,
    /// `truenorth://cockpit`, backed by `specs/release-plan.yaml`.
    Cockpit,
    /// `truenorth://conventions`, backed by `CONVENTIONS.md`.
    Conventions,
    /// `truenorth://ontology`, backed by `specs/ontology.yaml`.
    Ontology,
}

/// Every served resource, in list order.
pub const ALL_RESOURCES: [ResourceDoc; 4] = [
    ResourceDoc::State,
    ResourceDoc::Cockpit,
    ResourceDoc::Conventions,
    ResourceDoc::Ontology,
];

/// An error reading a resource (Requirement 5.7).
#[derive(Debug, PartialEq, Eq)]
pub enum ResourceReadError {
    /// The backing file is absent.
    NotFound(String),
    /// The backing file failed to parse or validate. The message names the file.
    Invalid(String),
}

impl ResourceDoc {
    /// Resolve a URI string to a resource.
    pub fn from_uri(uri: &str) -> Option<Self> {
        ALL_RESOURCES.into_iter().find(|doc| doc.uri() == uri)
    }

    /// The resource URI.
    pub fn uri(self) -> &'static str {
        match self {
            ResourceDoc::State => "truenorth://state",
            ResourceDoc::Cockpit => "truenorth://cockpit",
            ResourceDoc::Conventions => "truenorth://conventions",
            ResourceDoc::Ontology => "truenorth://ontology",
        }
    }

    /// The resource's programmatic name.
    pub fn name(self) -> &'static str {
        match self {
            ResourceDoc::State => "state",
            ResourceDoc::Cockpit => "cockpit",
            ResourceDoc::Conventions => "conventions",
            ResourceDoc::Ontology => "ontology",
        }
    }

    /// The resource's MIME type.
    pub fn mime_type(self) -> &'static str {
        match self {
            ResourceDoc::Conventions => "text/markdown",
            _ => "application/yaml",
        }
    }

    /// The backing file path under a repository root.
    pub fn backing_path(self, repo_root: &std::path::Path) -> PathBuf {
        match self {
            ResourceDoc::State => repo_root.join("specs").join("state.yaml"),
            ResourceDoc::Cockpit => repo_root.join("specs").join("release-plan.yaml"),
            ResourceDoc::Conventions => repo_root.join("CONVENTIONS.md"),
            ResourceDoc::Ontology => repo_root.join("specs").join("ontology.yaml"),
        }
    }

    /// Read and validate the backing file's current content (Requirement 5.5).
    ///
    /// State and release-plan validate against the observed schemas; ontology parses as a
    /// YAML document; conventions is returned as raw markdown.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceReadError::NotFound`] when the file is absent, or
    /// [`ResourceReadError::Invalid`] when it fails to parse or validate (Requirement
    /// 5.7).
    pub fn read_current(self, repo_root: &std::path::Path) -> Result<String, ResourceReadError> {
        let path = self.backing_path(repo_root);
        let text = std::fs::read_to_string(&path)
            .map_err(|_| ResourceReadError::NotFound(display_backing(self)))?;

        match self {
            ResourceDoc::State => {
                validate_state(&text).map_err(|e| ResourceReadError::Invalid(e.to_string()))?
            }
            ResourceDoc::Cockpit => {
                validate_release_plan(&text)
                    .map_err(|e| ResourceReadError::Invalid(e.to_string()))?;
                // The cockpit resource serves the release plan document as-is.
                return Ok(text);
            }
            ResourceDoc::Ontology => {
                serde_yaml::from_str::<serde_yaml::Value>(&text).map_err(|e| {
                    ResourceReadError::Invalid(format!("ontology.yaml failed to parse: {e}"))
                })?;
                return Ok(text);
            }
            // Conventions is free-form markdown, so it has no schema to validate.
            ResourceDoc::Conventions => return Ok(text),
        };
        Ok(text)
    }
}

/// The backing file name for an error message.
fn display_backing(doc: ResourceDoc) -> String {
    match doc {
        ResourceDoc::State => "specs/state.yaml".to_string(),
        ResourceDoc::Cockpit => "specs/release-plan.yaml".to_string(),
        ResourceDoc::Conventions => "CONVENTIONS.md".to_string(),
        ResourceDoc::Ontology => "specs/ontology.yaml".to_string(),
    }
}

/// The last-good content cache (Requirement 5.7).
///
/// A successful read updates the cache. A failed read leaves it intact, so the last
/// successfully parsed content is retained in memory while the file is broken on disk.
#[derive(Debug, Default)]
pub struct ResourceCache {
    last_good: Mutex<HashMap<ResourceDoc, String>>,
}

impl ResourceCache {
    /// A fresh, empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read a resource, updating the cache on success (Requirements 5.5, 5.7).
    ///
    /// # Errors
    ///
    /// Returns the [`ResourceReadError`] from [`ResourceDoc::read_current`]. The cache is
    /// left intact on error, so a later reader can still fetch the last-good content.
    pub fn read(
        &self,
        doc: ResourceDoc,
        repo_root: &std::path::Path,
    ) -> Result<String, ResourceReadError> {
        match doc.read_current(repo_root) {
            Ok(content) => {
                self.store(doc, &content);
                Ok(content)
            }
            Err(error) => Err(error),
        }
    }

    /// The last-good content for a resource, when one has been read successfully.
    // Read by the resource tests and by the server wiring in task 15. Remove this allow
    // once task 15 consumes it in the running server.
    #[allow(dead_code)]
    pub fn last_good(&self, doc: ResourceDoc) -> Option<String> {
        self.last_good
            .lock()
            .expect("cache lock")
            .get(&doc)
            .cloned()
    }

    /// Store a resource's content as the last-good value.
    fn store(&self, doc: ResourceDoc, content: &str) {
        self.last_good
            .lock()
            .expect("cache lock")
            .insert(doc, content.to_string());
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
