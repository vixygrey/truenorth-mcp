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

use std::path::PathBuf;

use crate::engine::agent_ws::fs_boundary::RepositoryFs;
use crate::engine::features::Features;
use crate::engine::validate::{validate_release_plan, validate_state};

pub mod adr;
mod cache;
pub mod cockpit;
pub mod ontology;

pub use cache::ResourceCache;

/// A served resource, identified by its `truenorth://` URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceDoc {
    /// `truenorth://state`, backed by `.agent/tasks/state.json`.
    State,
    /// `truenorth://cockpit`, backed by `.agent/tasks/release-plan.json`.
    Cockpit,
    /// `truenorth://conventions`, backed by `CONVENTIONS.md`.
    Conventions,
    /// `truenorth://ontology`, backed by `.agent/ontology.yml`.
    Ontology,
    /// `truenorth://adr`, a read-only view over `specs/adr/` (Requirement 9.1, 9.2).
    Adr,
}

/// Every served resource, in list order.
pub const ALL_RESOURCES: [ResourceDoc; 5] = [
    ResourceDoc::State,
    ResourceDoc::Cockpit,
    ResourceDoc::Conventions,
    ResourceDoc::Ontology,
    ResourceDoc::Adr,
];

/// The resources served under the resolved feature flags (Requirement 3.1, 3.2).
///
/// When the ontology feature is disabled, `truenorth://ontology` is omitted, so it is
/// neither listed nor resolvable. A read of it is then an unknown resource (Requirement
/// 3.3), and no ontology file is seeded (Requirement 3.4). Every other resource is served
/// unchanged (Requirement 3.6).
pub fn served_resources(features: Features) -> Vec<ResourceDoc> {
    ALL_RESOURCES
        .into_iter()
        .filter(|doc| *doc != ResourceDoc::Ontology || features.ontology)
        .collect()
}

/// Resolve a URI to a served resource under the flags (Requirement 3.3).
///
/// Returns `None` for `truenorth://ontology` when the ontology feature is disabled, so the
/// server maps it to the unknown-resource error, exactly as it does for any unknown URI.
pub fn served_from_uri(uri: &str, features: Features) -> Option<ResourceDoc> {
    ResourceDoc::from_uri(uri).filter(|doc| *doc != ResourceDoc::Ontology || features.ontology)
}

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
            ResourceDoc::Adr => "truenorth://adr",
        }
    }

    /// The resource's programmatic name.
    pub fn name(self) -> &'static str {
        match self {
            ResourceDoc::State => "state",
            ResourceDoc::Cockpit => "cockpit",
            ResourceDoc::Conventions => "conventions",
            ResourceDoc::Ontology => "ontology",
            ResourceDoc::Adr => "adr",
        }
    }

    /// The resource's MIME type.
    pub fn mime_type(self) -> &'static str {
        match self {
            ResourceDoc::State | ResourceDoc::Cockpit => "application/json",
            ResourceDoc::Conventions | ResourceDoc::Adr => "text/markdown",
            ResourceDoc::Ontology => "application/yaml",
        }
    }

    /// The backing file path under a repository root.
    ///
    /// The cockpit files live under `.agent/` (Requirements 2.1, 2.2, 2.3). Conventions
    /// stays at the repository root.
    pub fn backing_path(self, repo_root: &std::path::Path) -> PathBuf {
        match self {
            ResourceDoc::State => repo_root.join(".agent").join("tasks").join("state.json"),
            ResourceDoc::Cockpit => repo_root
                .join(".agent")
                .join("tasks")
                .join("release-plan.json"),
            ResourceDoc::Conventions => repo_root.join("CONVENTIONS.md"),
            ResourceDoc::Ontology => repo_root.join(".agent").join("ontology.yml"),
            // The ADR resource is backed by a directory of human-authored files under
            // `specs/`, not a single file (Requirement 9.1, 9.2).
            ResourceDoc::Adr => repo_root.join("specs").join("adr"),
        }
    }

    /// The legacy ontology backing path.
    pub fn legacy_backing_path(self, repo_root: &std::path::Path) -> Option<PathBuf> {
        match self {
            ResourceDoc::Ontology => Some(repo_root.join("specs").join("ontology.yaml")),
            ResourceDoc::State
            | ResourceDoc::Cockpit
            | ResourceDoc::Conventions
            | ResourceDoc::Adr => None,
        }
    }

    /// Whether reading this resource would create the ontology backing file.
    pub(crate) fn needs_seed(self, repo_root: &std::path::Path) -> bool {
        if self != ResourceDoc::Ontology {
            return false;
        }
        let Ok(fs) = RepositoryFs::open(repo_root) else {
            return false;
        };
        [".agent/ontology.yml", "specs/ontology.yaml"]
            .iter()
            .all(|path| {
                fs.open_read(std::path::Path::new(path))
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            })
    }

    /// Read and validate the backing file's current content (Requirement 5.5).
    ///
    /// State and release-plan are JSON-only; ontology may fall back to its legacy YAML
    /// path. Conventions is returned as raw markdown.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceReadError::NotFound`] when the file is absent, or
    /// [`ResourceReadError::Invalid`] when it fails to parse or validate (Requirement
    /// 5.7).
    pub fn read_current(self, repo_root: &std::path::Path) -> Result<String, ResourceReadError> {
        // The ADR resource reads a directory of human-authored files, not a single backing
        // file, so it resolves through its own reader (Requirement 9.2).
        if self == ResourceDoc::Adr {
            return adr::read_adr_dir(&self.backing_path(repo_root));
        }

        // Only an absent primary ontology may use the legacy path or create a seed.
        // A symlink or forbidden file is an error, never a reason to try a fallback.
        let fs = RepositoryFs::open(repo_root)
            .map_err(|e| ResourceReadError::Invalid(format!("could not open repository: {e}")))?;
        let primary = self.backing_path(repo_root);
        let relative = primary.strip_prefix(repo_root).map_err(|e| {
            ResourceReadError::Invalid(format!("could not resolve {}: {e}", display_backing(self)))
        })?;
        let mut file = match fs.open_read(relative) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(legacy) = self.legacy_backing_path(repo_root) {
                    let legacy_relative = legacy.strip_prefix(repo_root).map_err(|e| {
                        ResourceReadError::Invalid(format!(
                            "could not resolve legacy ontology: {e}"
                        ))
                    })?;
                    match fs.open_read(legacy_relative) {
                        Ok(file) => file,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            create_ontology_on_read(repo_root)?;
                            fs.open_read(relative).map_err(|e| read_error(self, e))?
                        }
                        Err(e) => {
                            return Err(ResourceReadError::Invalid(format!(
                                "could not read specs/ontology.yaml: {e}"
                            )));
                        }
                    }
                } else {
                    return Err(ResourceReadError::NotFound(display_backing(self)));
                }
            }
            Err(e) => return Err(read_error(self, e)),
        };

        let mut text = String::new();
        use std::io::Read;
        file.read_to_string(&mut text)
            .map_err(|e| read_error(self, e))?;

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
                crate::engine::yaml::from_str::<saneyaml::Value>(&text).map_err(|e| {
                    ResourceReadError::Invalid(format!(".agent/ontology.yml failed to parse: {e}"))
                })?;
                return Ok(text);
            }
            // Conventions is free-form markdown, so it has no schema to validate.
            ResourceDoc::Conventions => return Ok(text),
            // Adr returns early above through read_adr_dir, so it does not reach this
            // match. Return a typed error rather than panic, so a future change that
            // breaks the early return degrades to an error instead of a crash.
            ResourceDoc::Adr => {
                return Err(ResourceReadError::Invalid(
                    "internal: the ADR resource must read through read_adr_dir".to_string(),
                ));
            }
        };
        Ok(text)
    }
}

/// The minimal ontology document seeded on first read (Requirement 2.4).
///
/// It is the shared empty stub from [`crate::engine::spec::Ontology::empty_stub`],
/// serialized to YAML. Defining the stub once means the seed and the generate-tool
/// overwrite check cannot drift (Requirement 4.9). The generate tool replaces it with a
/// full ontology. The resource parses it as a YAML document, so this stub reads back
/// cleanly.
fn ontology_seed() -> Result<String, ResourceReadError> {
    crate::engine::yaml::to_string(&crate::engine::spec::Ontology::empty_stub()).map_err(|e| {
        ResourceReadError::Invalid(format!("could not serialize the ontology stub: {e}"))
    })
}

/// Create the ontology backing file under `.agent/` on first read (Requirements 2.4, 2.5).
///
/// The descriptor-relative, no-clobber write keeps a concurrently created file intact.
/// A failure leaves the last good content cached (Requirement 2.5).
fn create_ontology_on_read(repo_root: &std::path::Path) -> Result<PathBuf, ResourceReadError> {
    let fs = RepositoryFs::open(repo_root)
        .map_err(|e| ResourceReadError::Invalid(format!("could not open repository: {e}")))?;
    fs.create_dir_all(std::path::Path::new(".agent"))
        .map_err(|e| ResourceReadError::Invalid(format!("could not create .agent/: {e}")))?;
    let relative = std::path::Path::new(".agent/ontology.yml");
    let path = repo_root.join(relative);
    let seed = ontology_seed()?;
    match fs.write_new(relative, seed.as_bytes()) {
        Ok(()) => Ok(path),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(path),
        Err(e) => Err(ResourceReadError::Invalid(format!(
            "could not create .agent/ontology.yml: {e}. No file was created."
        ))),
    }
}

/// Map a guarded read failure to the resource's file name.
fn read_error(doc: ResourceDoc, error: std::io::Error) -> ResourceReadError {
    let backing = display_backing(doc);
    match error.kind() {
        std::io::ErrorKind::NotFound => ResourceReadError::NotFound(backing),
        _ => ResourceReadError::Invalid(format!(
            "could not read {backing}: {error}. Check the file permissions and \
             that it is a regular file."
        )),
    }
}

/// The backing file name for an error message.
fn display_backing(doc: ResourceDoc) -> String {
    match doc {
        ResourceDoc::State => ".agent/tasks/state.json".to_string(),
        ResourceDoc::Cockpit => ".agent/tasks/release-plan.json".to_string(),
        ResourceDoc::Conventions => "CONVENTIONS.md".to_string(),
        ResourceDoc::Ontology => ".agent/ontology.yml".to_string(),
        ResourceDoc::Adr => "specs/adr/".to_string(),
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;

// Property tests (Property 7) live in a separate sibling so the example-based unit tests
// stay focused. The `#[path]` include keeps them a child module of the resources module.
#[cfg(test)]
#[path = "resources_prop_tests.rs"]
mod prop_tests;
