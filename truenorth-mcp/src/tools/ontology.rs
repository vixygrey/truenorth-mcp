//! Ontology tools: `truenorth_generate_ontology` and `truenorth_verify_ontology`.
//!
//! `truenorth_generate_ontology` seeds `.agent/ontology.yml` with the domain, a
//! timestamp, the baseline global constraints, and an entities scaffold (Requirements
//! 4.1, 4.3, 9.9). It writes through the single write guard and overwrites only the empty
//! stub. It rejects invalid input without writing (Requirement
//! 4.2) and refuses to overwrite an existing file (Requirement 9.10).
//!
//! `truenorth_verify_ontology` scans code against the ontology (Requirements 4.7, 4.8,
//! 4.10). Without explicit `scope_paths` it scans the git-changed files in scope. An
//! empty scope passes with zero files. No violations passes. Otherwise it returns the
//! formatted first violation.
//!
//! Requirements: 4.1, 4.2, 4.3, 4.4, 4.7, 4.8, 4.10, 9.9, 9.10. Design: Part II §2, §6.

use std::path::{Path, PathBuf};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::engine::git::changed_files_in_scope;
use crate::engine::ontology_scan::{Violation, pick_analyzer};
use crate::engine::spec::{Constraint, Entity, Ontology};
use crate::server::TrueNorthServer;

/// The maximum length of the domain name (Requirement 4.1).
const MAX_DOMAIN: usize = 200;

/// The `truenorth_generate_ontology` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateOntologyArgs {
    /// The domain name (1 to 200 chars).
    pub domain: String,
    /// The PRDs, spec files, or code directories to model (at least one entry).
    pub source_paths: Vec<String>,
}

/// The `truenorth_verify_ontology` input contract (design §2).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VerifyOntologyArgs {
    /// The paths to scan. Defaults to the git-changed files in scope when omitted.
    #[serde(default)]
    pub scope_paths: Option<Vec<String>>,
}

#[tool_router(router = ontology_router, vis = "pub")]
impl TrueNorthServer {
    /// Seed `.agent/ontology.yml` from the domain and source paths.
    #[tool(
        description = "Seed .agent/ontology.yml with the domain, baseline constraints, and an entities scaffold."
    )]
    pub async fn truenorth_generate_ontology(
        &self,
        params: Parameters<GenerateOntologyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = params.0;

        // Validate input before any write (Requirement 4.2).
        let domain_len = args.domain.chars().count();
        if !(1..=MAX_DOMAIN).contains(&domain_len) {
            return Err(ErrorData::invalid_params(
                format!("`domain` must be 1 to {MAX_DOMAIN} characters"),
                None,
            ));
        }
        if args.source_paths.is_empty() {
            return Err(ErrorData::invalid_params(
                "`source_paths` must contain at least one entry".to_string(),
                None,
            ));
        }

        // Overwrite only the empty stub. A real ontology is left for the human to edit
        // (Requirement 4.5, 4.6, 4.7). The resource may seed the stub first, so the stub
        // must remain overwritable.
        let primary = ontology_path(&self.ctx.repo_root);
        if primary.is_file() {
            let existing = read_and_parse(&primary)?;
            if !existing.is_empty_stub() {
                return Err(ErrorData::invalid_request(
                    ".agent/ontology.yml already exists. Edit it directly rather than \
                     regenerating."
                        .to_string(),
                    None,
                ));
            }
        }

        let ontology = seed_ontology(&args.domain, &args.source_paths);
        let yaml = serde_yaml::to_string(&ontology).map_err(|e| {
            ErrorData::internal_error(format!("could not serialize the ontology: {e}"), None)
        })?;

        // Write through the single guard, so the target stays under `.agent/`
        // (Requirement 4.2, 4.4).
        let rel = std::path::Path::new("ontology.yml");
        crate::engine::agent_ws::write_under_agent(&self.ctx.repo_root, rel, &yaml).map_err(
            |e| {
                ErrorData::internal_error(
                    format!("could not write .agent/ontology.yml: {e}. No file was created."),
                    None,
                )
            },
        )?;

        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::json!({
                "domain": args.domain,
                "path": ".agent/ontology.yml",
                "entities": ontology.entities.len(),
                "constraints": ontology.constraints.len(),
            })
            .to_string(),
        )]))
    }

    /// Scan code against the ontology and report the first violation, if any.
    #[tool(description = "Scan code for prohibited aliases against .agent/ontology.yml.")]
    pub async fn truenorth_verify_ontology(
        &self,
        params: Parameters<VerifyOntologyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let ontology = self.read_ontology()?;

        // A not-yet-defined ontology passes with an informational note, so a pass against
        // an empty ontology does not read as false assurance (Requirement 4.10).
        if ontology.is_empty_stub() {
            return Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({
                    "passed": true,
                    "files_scanned": 0,
                    "note": "The ontology is not yet defined (empty stub). \
                             Run truenorth_generate_ontology to define it."
                })
                .to_string(),
            )]));
        }

        let scope = self.resolve_scope(params.0.scope_paths)?;

        // Empty scope passes with zero files scanned (Requirement 4.8).
        if scope.is_empty() {
            return Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({ "passed": true, "files_scanned": 0 }).to_string(),
            )]));
        }

        let violations = scan_scope(&self.ctx.repo_root, &scope, &ontology);

        match violations.first() {
            // No violations passes (Requirement 4.10).
            None => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({ "passed": true, "files_scanned": scope.len() }).to_string(),
            )])),
            // Otherwise return the formatted first violation.
            Some(first) => Err(ErrorData::invalid_request(
                first.message.clone(),
                Some(serde_json::json!({ "violations": violations.len() })),
            )),
        }
    }
}

impl TrueNorthServer {
    /// Read and parse the ontology, erroring when it is absent or malformed.
    ///
    /// Prefers `.agent/ontology.yml` and falls back to a legacy `specs/ontology.yaml`
    /// (Requirement 4.3).
    fn read_ontology(&self) -> Result<Ontology, ErrorData> {
        let path = ontology_read_path(&self.ctx.repo_root).ok_or_else(|| {
            ErrorData::invalid_request(
                ".agent/ontology.yml not found. Run truenorth_generate_ontology first.".to_string(),
                None,
            )
        })?;
        read_and_parse(&path)
    }

    /// Resolve the scan scope: explicit paths, else the git-changed files in scope
    /// (Requirement 4.7).
    fn resolve_scope(&self, scope_paths: Option<Vec<String>>) -> Result<Vec<PathBuf>, ErrorData> {
        match scope_paths {
            Some(paths) => Ok(paths.into_iter().map(PathBuf::from).collect()),
            None => changed_files_in_scope(&self.ctx.repo_root).map_err(|e| {
                ErrorData::internal_error(
                    format!("could not resolve the git-changed scope: {e}"),
                    None,
                )
            }),
        }
    }
}

/// The primary ontology backing file, matching the resource (Requirement 4.1).
fn ontology_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".agent").join("ontology.yml")
}

/// The legacy ontology backing file, a read-only fallback (Requirement 4.3).
fn legacy_ontology_path(repo_root: &Path) -> PathBuf {
    repo_root.join("specs").join("ontology.yaml")
}

/// Resolve the read path: `.agent/ontology.yml`, else the legacy `specs/ontology.yaml`.
///
/// Returns `None` when neither exists. A write always targets `.agent/`, so the legacy
/// file is never mutated (Requirement 4.4).
fn ontology_read_path(repo_root: &Path) -> Option<PathBuf> {
    let primary = ontology_path(repo_root);
    if primary.is_file() {
        return Some(primary);
    }
    let legacy = legacy_ontology_path(repo_root);
    legacy.is_file().then_some(legacy)
}

/// Read and parse an ontology file into an [`Ontology`], mapping errors to MCP errors.
fn read_and_parse(path: &Path) -> Result<Ontology, ErrorData> {
    let text = std::fs::read_to_string(path).map_err(|_| {
        ErrorData::invalid_request(
            ".agent/ontology.yml not found. Run truenorth_generate_ontology first.".to_string(),
            None,
        )
    })?;
    serde_yaml::from_str(&text).map_err(|e| {
        ErrorData::invalid_request(format!(".agent/ontology.yml failed to parse: {e}"), None)
    })
}

/// Build the seed ontology from the domain and source paths.
///
/// The seed carries the domain, a timestamp, the baseline global constraints, and one
/// scaffold entity per source path so the human has a concrete starting point to fill in.
/// The tool does not infer real domain entities from the sources; it seeds a valid file
/// for the human and later analysis to build on.
fn seed_ontology(domain: &str, source_paths: &[String]) -> Ontology {
    Ontology {
        version: "1".to_string(),
        domain: domain.to_string(),
        last_updated: now_iso8601(),
        entities: source_paths.iter().map(scaffold_entity).collect(),
        constraints: baseline_constraints(),
    }
}

/// A scaffold entity derived from a source path, for the human to complete.
fn scaffold_entity(source: &String) -> Entity {
    Entity {
        name: entity_name_from(source),
        description: format!("Scaffold entity derived from {source}. Replace with the real model."),
        primary_key: "id".to_string(),
        invariants: Vec::new(),
        states: Vec::new(),
        transitions: std::collections::BTreeMap::new(),
        prohibited_aliases: Vec::new(),
    }
}

/// A PascalCase-ish entity name from a source path's file stem.
fn entity_name_from(source: &str) -> String {
    Path::new(source)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Entity".to_string())
}

/// The baseline global constraints (design §3.2 example).
fn baseline_constraints() -> Vec<Constraint> {
    vec![
        Constraint {
            id: "C-01".to_string(),
            rule: "Soft deletion uses deleted_at, never a boolean flag.".to_string(),
        },
        Constraint {
            id: "C-02".to_string(),
            rule: "Boolean state flags (is_*) are prohibited; model states explicitly.".to_string(),
        },
    ]
}

/// The current time as an ISO-8601 UTC string, seconds precision.
///
/// The date is computed from the unix epoch with the days-from-civil algorithm, so it is
/// correct and dependency-free.
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    let sod = secs % 86_400;
    let (hour, minute, second) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Convert a day count since the unix epoch to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (year + i64::from(month <= 2), month, day)
}

/// Scan every file in scope, collecting violations in order.
fn scan_scope(repo_root: &Path, scope: &[PathBuf], ontology: &Ontology) -> Vec<Violation> {
    let mut violations = Vec::new();
    for rel in scope {
        let full = repo_root.join(rel);
        let Ok(contents) = std::fs::read_to_string(&full) else {
            continue;
        };
        let analyzer = pick_analyzer(rel);
        violations.extend(analyzer.scan(rel, &contents, ontology));
    }
    violations
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `ontology`.
#[cfg(test)]
#[path = "ontology_tests.rs"]
mod tests;
