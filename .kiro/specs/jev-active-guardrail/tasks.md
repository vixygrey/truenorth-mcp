# Implementation Plan: Jev Active Guardrail

## Overview

This plan wires the Jev evaluation harness into the live request path as an active
guardrail. The guardrail runs a pre-write check that returns one decision: allow,
block, or annotate. It composes the existing `engine::jev` harness modules whole
(`drift`, `rigor`, `secret_filter`, `confidence`, `config`, `client_fake`,
`client_http`). It adds no new Jev client, no new wire type, and no new aspect
evaluator (ADR-G1).

The runtime crate is written in **Rust** (rmcp, serde, serde_json, schemars,
thiserror, tokio, proptest). The optional `reqwest` dependency provides the HTTP
transport, gated by the Cargo `jev-http` feature. The design pins concrete Rust
throughout, so no pseudocode language selection is required.

The feature adds four new items: a new `engine::jev::guard` module with the decision
logic, a `tools::guard` tool (`truenorth_guard_change`), a `guard` CLI subcommand on
the `jev-bench` bin behind `jev-http`, and a `PreToolUse` hook emitted by the
scaffold. Each item composes existing harness modules.

Two independent layers gate the guardrail. The runtime `jev` flag defaults off. The
Cargo `jev-http` feature gates the networking dependency. The deterministic layer
compiles and runs without the feature and never fails open. The probabilistic layer
compiles only under `jev-http` and always fails open. The default build and
`cargo test` compile no HTTP client, so the suite runs offline against the named fake
(R8.2, R8.4, R8.5).

Tasks build in the design's layer order. The guard types and the deterministic layer
land first. The pure combine step and the confidence gate land next. The
`evaluate_guard` function and the probabilistic fail-open layer land after that. The
tool, the CLI subcommand, and the emitted hook land last. The pure combine step holds
the confidence gate and the determinism, so its property tests run runtime-free.

Every staged public item carries `#[cfg_attr(not(test), allow(dead_code))]` with a
reason and is exercised by a test. Fakes are named types. Library code raises no
panic, and calls no `unwrap` or `expect`. Each file stays under about 300 lines.

## Tasks

- [ ] 1. Guard types and the deterministic layer
  - [ ] 1.1 Define the guard types in `engine/jev/guard.rs`
    - Register `pub mod guard;` in `runtime/src/engine/jev/mod.rs`
    - Define `ProposedChange { paths: Vec<String>, content: String }` with `serde::Deserialize` and `schemars::JsonSchema` (R1.2)
    - Define `GuardDecision` with `Allow { notes: Vec<String> }`, `Block(NeutralizationPacket)`, and `Annotate { notes: Vec<String> }` (R1.4)
    - Define `NeutralizationPacket { violated_check, offending_value: Option<String>, remediation, suggested_fix: Option<String> }` with `serde::Serialize` (R6.1, R6.4)
    - Carry `#[cfg_attr(not(test), allow(dead_code))]` with a reason on each staged public item
    - _Requirements: 1.2, 1.4, 6.1, 6.4_

  - [ ] 1.2 Implement the deterministic layer in `engine/jev/guard.rs`
    - Match a written path against the protected paths through `jev::drift::path_is_protected`; return `Block` with a `protected-path` packet naming the path and no Jev call (R2.1, R2.6, R6.4)
    - Scan the content against the secret denylist through `config::secret_denylist`; return `Block` with a `secret` packet naming the matched pattern name and no Jev call (R2.2, R6.4)
    - Name never the secret value, only the pattern name (R6.3)
    - Run the deterministic layer regardless of the flag and return the same result on the same input (R2.3, R2.5)
    - _Requirements: 2.1, 2.2, 2.3, 2.5, 2.6, 6.3, 6.4_

  - [ ] 1.3 Write property test for deterministic protected-path block
    - **Property 34: Deterministic protected-path block**
    - Drive with generated changes that write a protected path, the flag on and off, and the named fake; assert `Block` with zero fake calls, regardless of the flag; minimum 100 iterations
    - **Validates: Requirements 2.1, 2.3, 2.4**

  - [ ] 1.4 Write property test for deterministic secret block
    - **Property 35: Deterministic secret block**
    - Drive with generated content that injects denylist hits and the named fake; assert `Block` and zero fake calls; minimum 100 iterations
    - **Validates: Requirements 2.2, 3.3**

- [ ] 2. The pure combine step and the confidence gate
  - [ ] 2.1 Implement the combine step in `engine/jev/guard.rs`
    - Implement a pure `combine(rigor, drift, config) -> GuardDecision` over its inputs, with no async and no state read (P39)
    - Return a `drift` `Block` when the drift Noul is out of scope and confident at the configured boundary (R3.4, R5.1)
    - Return a `rigor` `Block` with a suggested fix when a rigor failure is confident enough to block; return `Annotate` with the summary and the suggested fix otherwise (R6.2, R5.1, R5.2)
    - Read every threshold from `JevConfig` and apply the higher `destructive_threshold` for a destructive candidate; hardcode no threshold value (R5.3, R5.4)
    - Return `Allow` with empty notes when no candidate blocks (R1.4)
    - _Requirements: 3.4, 5.1, 5.2, 5.3, 5.4, 6.2_

  - [ ] 2.2 Write property test for the confidence-gated block
    - **Property 38: Confidence-gated block**
    - Drive `combine` with generated candidate signals and valid thresholds; assert `Block` exactly when the confidence is at or above the threshold; minimum 100 iterations
    - **Validates: Requirements 5.1, 5.2**

  - [ ] 2.3 Write property test for decision determinism
    - **Property 39: Decision determinism**
    - Drive `combine` with generated signals and thresholds; assert two evaluations of the same input return the same decision; minimum 100 iterations
    - **Validates: Requirements 5.5**

- [ ] 3. The evaluate_guard probabilistic layer and fail-open
  - [ ] 3.1 Implement `evaluate_guard` in `engine/jev/guard.rs`
    - Implement `evaluate_guard<C: JevClient>(change, protected_paths, config, jev_enabled, client: Option<&C>) -> GuardDecision`
    - Run the deterministic layer first, then the probabilistic layer (R1.3)
    - Return `Allow` with a note when the flag is off or the client is absent, with no Jev call (R4.1)
    - Return `Allow` with a note when the API key is absent, with no Jev call (R4.2)
    - Build the Jev state through `secret_filter::build_state_with_secret_filter`; surface `SecretResidual` and make no call on a residual (R3.2, R3.3)
    - Score rigor and drift through the reused harness aspects and keep each request at or under the token budget (R3.1, R3.5)
    - Return `Allow` with a note naming the cause on a timeout, a network error, or a non-success status; never `Block` on unavailability (R4.3, R4.5)
    - Return the combine result when both aspects succeed (R1.4)
    - _Requirements: 1.3, 1.4, 3.1, 3.2, 3.3, 3.5, 4.1, 4.2, 4.3, 4.5_

  - [ ] 3.2 Write property test for flag-off silence
    - **Property 36: Flag-off silence**
    - Drive with generated changes, the flag off, and the named fake; assert no Jev call and `Allow` with a note; assert the fake records zero calls; minimum 100 iterations
    - **Validates: Requirements 4.1, 8.2**

  - [ ] 3.3 Write property test for probabilistic fail-open
    - **Property 37: Probabilistic fail-open**
    - Drive the named fake with timeout, network error, non-success status, and absent key; assert `Allow`, never `Block`; minimum 100 iterations
    - **Validates: Requirements 4.2, 4.3, 4.5**

  - [ ] 3.4 Write property test for no secret to Jev
    - **Property 40: No secret to Jev**
    - Drive with generated changes that inject denylist content; assert no Jev request state and no `NeutralizationPacket` carries denylist content, and no output carries the API key; minimum 100 iterations
    - **Validates: Requirements 3.2, 6.3**

  - [ ] 3.5 Write example tests for the decision boundaries
    - Test the deterministic block on a `specs/` write, a `LICENSE` write, and content hits on `.env`, `secret`, and `credentials` patterns
    - Test the confidence gate at exactly the threshold, just below, and with the destructive threshold for a destructive candidate
    - Test the flag-off note and the absent-key note
    - _Requirements: 2.1, 2.2, 5.1, 5.2, 5.3, 4.1, 4.2_

- [ ] 4. The truenorth_guard_change tool
  - [ ] 4.1 Implement the guard tool in `tools/guard.rs`
    - Register `pub mod guard;` in `runtime/src/tools/mod.rs`
    - Define `GuardChangeArgs { paths: Vec<String>, content: String }` with a strict `schemars` input schema (R1.1, R1.2)
    - Validate the input; return a typed invalid-params error naming the offending field and run no check on a malformed input (R1.5)
    - Resolve the protected paths, the config, the feature flag, and the client, then call `evaluate_guard` (R1.3)
    - Map an `Allow` or an `Annotate` to a success result carrying the notes; map a `Block` to an MCP error carrying the `NeutralizationPacket` (R1.4)
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

  - [ ] 4.2 Register the guard tool in the router
    - Add the guard router to the tool-router assembly in `runtime/src/server.rs` alongside the existing tools
    - Leave every other tool unchanged (R1.6)
    - _Requirements: 1.6_

  - [ ] 4.3 Write unit tests for the tool mapping
    - Test that an `Allow` maps to a success result carrying the notes and a `Block` maps to an MCP error carrying the packet
    - Test that a malformed input returns a typed error naming the field with no check run
    - Drive with the named fake, so the test makes no network call
    - _Requirements: 1.4, 1.5, 8.2_

- [ ] 5. Checkpoint - Make sure that the core and the tool tests pass
  - Make sure that all tests pass offline against the fake with no `jev-http` feature. Ask the user if questions arise.

- [ ] 6. The guard CLI subcommand behind jev-http
  - [ ] 6.1 Implement the guard subcommand on the jev-bench bin
    - Add a `guard` subcommand to the existing `jev-bench` bin, gated behind the `jev-http` feature (ADR-G4)
    - Read the proposed change on standard input and call `evaluate_guard` (R7.1)
    - Print the block reason to standard error and exit non-zero on a `Block`; exit zero on an `Allow` (R7.2, R7.3)
    - Run the deterministic layer even in the default build, so a protected-path or a secret block holds without the feature (R7.5, R8.4)
    - Leak no API key and no denylist content to standard output or standard error (R7.6)
    - Carry no `#[test]`, so `cargo test` does not run the bin (R8.2)
    - _Requirements: 7.1, 7.2, 7.3, 7.5, 7.6, 8.4_

  - [ ] 6.2 Write property test for the hook exit code
    - **Property 41: Hook exit code**
    - Drive the CLI guard entry point with a deterministic block and an allow; assert a non-zero exit on the block and a zero exit on the allow; minimum 100 iterations
    - **Validates: Requirements 7.2, 7.3**

- [ ] 7. The emitted PreToolUse hook from the scaffold
  - [ ] 7.1 Add the PreToolUse hook template and emit path
    - Add the `PreToolUse` hook template to `runtime/src/tools/scaffold/templates.rs`, matching a write tool and running the guard CLI on standard input (R7.1)
    - Add the emit call to `runtime/src/tools/scaffold/mod.rs` through the audited `write_repo_seed` path
    - Make no network call when the flag is off (R7.5)
    - State in the emitted hook documentation that automatic interception depends on the client honoring the `PreToolUse` hook, and that the guard tool is the fallback (R7.4, R9.4)
    - _Requirements: 7.1, 7.4, 7.5, 9.4_

  - [ ] 7.2 Write unit tests for the emitted hook
    - Test that the scaffold emits the hook file with the write-tool matcher and the guard CLI command
    - Test that the emitted hook documentation states the client-honor boundary and the fallback
    - _Requirements: 7.1, 7.4, 9.4_

- [ ] 8. Structural and scope checks
  - [ ] 8.1 Write structural and scope smoke checks
    - Assert the guard CLI subcommand carries no `#[test]`, so `cargo test` does not run it (R8.2)
    - Assert `evaluate_guard` and the combine step hold no hardcoded threshold; the values come from `JevConfig` (R5.4)
    - Assert the default build compiles no reqwest for the guardrail (R8.5)
    - Assert the guard performs no write and triggers no secondary agent, and returns a suggested self-heal instruction only (R9.2, R9.3)
    - _Requirements: 5.4, 8.2, 8.5, 9.2, 9.3_

- [ ] 9. Final checkpoint - Full verification across both builds
  - Run `cargo fmt` and `cargo clippy` with warnings denied on the default build and on `--features jev-http`
  - Run `cargo test` offline and confirm the default build compiles no reqwest
  - Make sure that all tests pass across both builds. Ask the user if questions arise.

## Notes

- Every task in this plan is required, including the test tasks. There are no optional tasks. This matches the user's direction that every test task is implemented, not skipped.
- The guardrail composes the existing harness modules and adds no new Jev client, wire type, or aspect (ADR-G1).
- The deterministic layer never fails open. The probabilistic layer always fails open. This split is the load-bearing safety property of the feature (ADR-G3, P34, P35, P37).
- Two layers gate the guardrail: the runtime `jev` flag (default off) and the Cargo `jev-http` feature. The deterministic layer compiles and runs without the feature. The probabilistic layer and the CLI subcommand compile only under `jev-http`.
- The pure combine step (task 2.1) holds the confidence gate and the determinism, so P38 and P39 run runtime-free.
- No central `call_tool` seam exists, so the guard is an explicit tool plus the emitted hook (ADR-G2). No autonomous repair runs; the guard returns a suggested self-heal instruction only (ADR-G5, R9.2, R9.3).
- Each task references specific requirement clauses for traceability. Every property P34 to P41 maps to one property test at a minimum of 100 iterations, driven by the named fake client.
- Property to task map: P34 (1.3), P35 (1.4), P36 (3.2), P37 (3.3), P38 (2.2), P39 (2.3), P40 (3.4), P41 (6.2).

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1"] },
    { "id": 1, "tasks": ["1.2", "2.1"] },
    { "id": 2, "tasks": ["1.3", "1.4", "2.2", "2.3", "3.1"] },
    { "id": 3, "tasks": ["3.2", "3.3", "3.4", "3.5", "4.1"] },
    { "id": 4, "tasks": ["4.2", "6.1", "7.1"] },
    { "id": 5, "tasks": ["4.3", "6.2", "7.2", "8.1"] }
  ]
}
```
