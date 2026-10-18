//! Stdin proposals use the same decision path as the MCP guard, without starting a server.

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use truenorth_mcp::engine::features;
use truenorth_mcp::engine::jev::client_http::JevActivation;
use truenorth_mcp::engine::jev::config::{resolve, resolve_protected_paths};
use truenorth_mcp::engine::jev::guard::{GuardDecision, ProposedChange};
use truenorth_mcp::engine::jev::guard_cli::{block_message, exit_code_for};

// JSON escaping can expand a one-million-byte proposal to six million bytes.
const MAX_INPUT_BYTES: u64 = 8_000_000;

pub(super) fn run(root: &Path) -> ExitCode {
    let mut input = String::new();
    if std::io::stdin()
        .take(MAX_INPUT_BYTES + 1)
        .read_to_string(&mut input)
        .is_err()
    {
        return super::guard_fail(
            "could not read UTF-8 proposal JSON; supply paths, content, and optional task",
        );
    }
    if input.len() as u64 > MAX_INPUT_BYTES {
        return super::guard_fail(
            "proposal JSON exceeds 8000000 bytes; split the change into smaller proposals",
        );
    }
    let change: ProposedChange = match serde_json::from_str(&input) {
        Ok(change) => change,
        Err(_) => {
            return super::guard_fail(
                "invalid proposal JSON; supply an object with paths, content, and optional task, not a foreign tool payload",
            );
        }
    };
    if change.paths.is_empty() || change.paths.len() > 100 {
        return super::guard_fail("paths must contain 1 through 100 repository-relative targets");
    }
    if change.content.len() > 1_000_000 {
        return super::guard_fail(
            "content exceeds 1000000 UTF-8 bytes; split the change into smaller proposals",
        );
    }
    let config = match resolve(root) {
        Ok(config) => config,
        Err(error) => return super::guard_fail(&error.to_string()),
    };
    let protected = match resolve_protected_paths(root) {
        Ok(protected) => protected,
        Err(error) => return super::guard_fail(&error.to_string()),
    };
    let features = match features::resolve(root) {
        Ok(features) => features,
        Err(error) => return super::guard_fail(&error.to_string()),
    };
    let activation = if features.jev {
        match JevActivation::resolve() {
            Ok(activation) => Some(activation),
            Err(error) => return super::guard_fail(&error.to_string()),
        }
    } else {
        None
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(_) => {
            return super::guard_fail(
                "could not start the guard runtime; retry after resolving system resource limits",
            );
        }
    };
    match runtime.block_on(truenorth_mcp::tools::guard::run_accounted_decision(
        &change,
        &protected,
        &config,
        features.jev,
        activation.as_ref(),
        root,
    )) {
        Ok((decision, _accounting, _observation)) => report(&decision),
        Err(error) => super::guard_fail(&error.to_string()),
    }
}

fn report(decision: &GuardDecision) -> ExitCode {
    match decision {
        GuardDecision::Allow { notes } | GuardDecision::Annotate { notes } => {
            for note in notes {
                eprintln!("truenorth-mcp guard: {note}");
            }
        }
        GuardDecision::Block(_) => {
            if let Some(message) = block_message(decision) {
                eprintln!("truenorth-mcp guard: {message}");
            }
        }
    }
    ExitCode::from(exit_code_for(decision))
}
