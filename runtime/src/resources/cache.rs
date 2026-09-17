//! The last-good resource content cache (Requirement 5.7).
//!
//! [`ResourceCache`] holds the last successfully read content for each resource. A
//! successful read updates the cache; a failed read leaves it intact, so the last good
//! content is retained in memory while the file is broken on disk. The read-and-validate
//! logic lives in the parent module's [`ResourceDoc`]; this submodule is only the cache.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use super::{ResourceDoc, ResourceReadError};

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
    ///
    /// Exercised by the resource tests that assert the last-good retention on a failed
    /// read (Requirement 5.7). The server serves fresh reads and does not surface the
    /// cached value directly today, so this carries a non-test allow rather than deletion.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_good(&self, doc: ResourceDoc) -> Option<String> {
        // Recover from a poisoned lock rather than panic. The cached content is still
        // valid, so a prior panic elsewhere must not take down a resource read.
        self.last_good
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&doc)
            .cloned()
    }

    /// Store a resource's content as the last-good value.
    fn store(&self, doc: ResourceDoc, content: &str) {
        // Recover from a poisoned lock rather than panic (see `last_good`).
        self.last_good
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(doc, content.to_string());
    }
}
