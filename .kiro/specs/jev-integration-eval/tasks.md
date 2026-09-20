# Implementation Plan: Jev Integration Evaluation Harness

## Overview

This plan implements the Jev evaluation harness under `runtime/src/engine/jev/`.
The harness measures whether the TypeSafe Jev model fits five product aspects:
tool routing, parallel rigor scoring, drift guardrails, context pruning, and
self-healing decisions. The harness is engine infrastructure, not an MCP tool. It
registers nothing into the tool router, no gate, and no resource.

The runtime crate is written in **Rust** (rmcp, serde, serde_json, thiserror,
tokio, proptest). The optional `reqwest` dependency provides the HTTP transport.
The design pins concrete Rust throughout, so no pseudocode language selection is
required.

The whole capability sits behind two independent layers of opt-in. A runtime `jev`
flag in `.agent/config/rules.yml` gates behavior and defaults off (ADR-J1). A Cargo
`jev-http` feature gates the networking dependency and the `Http_Client` type. The
default build and the offline test suite compile no HTTP client, so `cargo test`
runs the full suite against the named `Fake_Client` (R2.4, R11.7, P22).

Tasks build in the design's layer order. The trait layer (the `Jev_Client` trait,
the wire types, and the `JevError` enum) lands first. The config, confidence, and
secret-filter modules land next, then the named `Fake_Client`. The five aspect
modules build on the trait and the fake. The `Http_Client` lands behind the
`jev-http` feature, with the pure status mapper split out so P32 runs offline. The
benchmark bin lands last behind `jev-http`. Every property test P22 to P33 runs
against the fake, offline.

Each new module keeps its tests in a sibling file included with the `#[path]`
pattern. Fakes are named types. Library code raises no panic, and calls no `unwrap`
or `expect`. Each file stays under about 300 lines.

## Tasks

- [x] 1. Cargo configuration and module registration
  - [x] 1.1 Add the `jev-http` feature and the optional reqwest dependency
    - Add a `jev-http` feature to `runtime/Cargo.toml` that maps to `["dep:reqwest"]`
    - Add `reqwest` as an optional dependency pinned to an exact version, `default-features = false`, features `["rustls-tls", "json"]`
    - Keep `default = []` so the default build and `cargo test` compile no networking
    - _Requirements: 2.4, 11.7, 2.7_

  - [x] 1.2 Register the harness module and the feature flag
    - Add `pub mod jev;` to `runtime/src/engine/mod.rs`
    - Add a `jev: bool` field to `Features` in `runtime/src/engine/features.rs`, resolved by the same reader as `ontology`
    - Use `#[serde(default)]` on the `jev` key so an absent key resolves to `false`, and set the `Features::default` `jev` value to `false` (R1.2, R1.3)
    - Extend the `features` block view so the same read path resolves both flags
    - _Requirements: 1.1, 1.2, 1.3, 1.8_

  - [x]* 1.3 Write unit tests for the flag resolution
    - Test that an absent `jev` key resolves to `false` and an absent `rules.yml` resolves to `false`
    - Test that a present-but-unreadable file returns the typed `Io` error and a present-but-unparsable file returns the typed `Parse` error, with no partial value
    - _Requirements: 1.2, 1.3, 1.6, 1.7_

- [x] 2. Trait layer: the Jev_Client trait, wire types, and JevError
  - [x] 2.1 Define the trait, the wire types, and the constants in `jev/mod.rs`
    - Define the `Jev_Client` trait with one async method `evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError>` (ADR-J2)
    - Define `JevRequest`, the `Question` enum (Noul, Choice, Score), `JevResponse`, the `Answer` enum (Noul, Choice, Score), and `Usage`, with serde internal `type` tags per the confirmed wire contract
    - Define the `JEV_MODEL = "jev-latest"`, `TOKEN_BUDGET = 32_000`, and `BYTES_PER_TOKEN` named constants
    - Register the child modules and compile the whole trait layer with no networking feature
    - _Requirements: 2.1, 2.10, 4.10, 6.5_

  - [x] 2.2 Define the JevError enum in `jev/mod.rs`
    - Define `JevError` with `thiserror`, one variant per failure the design names: `MissingApiKey`, `SecretResidual`, `Unauthorized`, `Validation`, `RateLimitedOrOverloaded`, `UnexpectedStatus`, `Network`, `Timeout`, `BudgetExceeded`, `MissingAnswer`, `DuplicateAnswer`, `UnexpectedOption`, `InvalidThresholds`, `InvalidPrice`, `InvalidFixtureSet`, `Config`
    - Write each message to name the offending value, the expected shape, and a remediation hint where one applies
    - Name nothing sensitive in `SecretResidual` and in every key-related variant (R9.6)
    - _Requirements: 2.6, 2.8, 9.4, 9.6, 10.1, 10.2, 10.5_

  - [x]* 2.3 Write unit tests for the wire-type serde round-trip
    - Test that each `Question` and `Answer` variant serializes and deserializes through its `type` tag
    - Test that a Score legend and probability map survive the round-trip
    - _Requirements: 2.1, 2.10_

- [x] 3. Config, confidence, and secret filter
  - [x] 3.1 Implement the config reader and validation in `jev/config.rs`
    - Define `JevConfig` (thresholds, boundaries, `retry_limit`, `max_backoff`, `timeout`, optional `price`) and `JevPrice`, read from a dedicated `jev` block of `.agent/config/rules.yml`
    - Resolve an absent block to the default, a present-but-unreadable file to the `Config` I/O error, and a present-but-unparsable file to the `Config` parse error, with no partial config
    - Validate the threshold pair: `confidence_low <= confidence_high`, both in 0 to 1; return `InvalidThresholds` on failure (R8.7)
    - Validate `destructive_threshold >= confidence_high` and in 0 to 1 (R8.8); validate every boundary in its stated range
    - Validate a price rate is finite and `>= 0`; return `InvalidPrice` naming the value on failure (R12.5)
    - _Requirements: 1.1, 8.6, 8.7, 8.8, 12.3, 12.5_

  - [x]* 3.2 Write unit tests for config parse and validation
    - Test the absent-block default, the unreadable-file error, and the unparsable-file error naming the path
    - Test the invalid threshold pair, the destructive-threshold rule, and the negative and non-numeric price rejections
    - Test the boundary values at 0.0, at 1.0, and at `low == high`
    - _Requirements: 1.6, 1.7, 8.7, 8.8, 12.5_

  - [x] 3.3 Implement confidence banding in `jev/confidence.rs`
    - Define the `ConfidenceBand` enum (High, Medium, Low)
    - Implement `confidence_band(confidence, low, high) -> Result<ConfidenceBand, JevError>` returning exactly one band for a valid pair and `InvalidThresholds` for an invalid pair (R8.1 to R8.4, R8.7)
    - Read no state, so the same inputs return the same band (R8.9)
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.7, 8.9_

  - [x]* 3.4 Write property test for confidence-band totality
    - **Property 23: Confidence-band totality**
    - Drive with generated confidence values and valid and invalid threshold pairs; assert exactly one band for a valid pair and the `InvalidThresholds` error for an invalid pair; minimum 100 iterations
    - **Validates: Requirements 8.1, 8.2, 8.3, 8.4, 8.7**

  - [x]* 3.5 Write property test for confidence-band determinism
    - **Property 24: Confidence-band determinism**
    - Drive with generated confidence values and threshold pairs; assert two resolutions of the same input return the same band; minimum 100 iterations
    - **Validates: Requirements 8.9**

  - [x] 3.6 Implement the secret filter in `jev/secret_filter.rs`
    - Implement `build_state_with_secret_filter(repo_root, paths)` that excludes a denylisted file by path through `config::is_secret_path`, redacts denylisted content through `config::secret_denylist()`, and re-checks the assembled serialized state
    - Return `SecretResidual` and make no call when the assembled state still matches the denylist (fail-closed)
    - Reuse the crate's already-compiled `secret_denylist()` regexes, so no regex compiles at call time
    - _Requirements: 9.1, 9.2, 9.7_

  - [x]* 3.7 Write property test for secret exclusion
    - **Property 31: Secret exclusion**
    - Drive with generated repository states that inject denylist content by path and by content; assert no assembled state matches the denylist, a residual match returns `SecretResidual` with no call, and no output carries the API key; minimum 100 iterations
    - **Validates: Requirements 9.1, 9.2, 9.6, 9.7**

- [ ] 4. Named Fake_Client
  - [ ] 4.1 Implement the Fake_Client in `jev/client_fake.rs`
    - Define `Fake_Client` as a named type that satisfies `Jev_Client` and returns configured responses with a ready future and no network call
    - Record each call so a test can assert the call count, for the flag-off silence property
    - Let a test set the queued `Answer` values per question id, so the fake drives every aspect and error path offline
    - _Requirements: 2.2, 2.4_

  - [ ]* 4.2 Write property test for flag-off silence
    - **Property 22: Flag-off silence**
    - Drive with generated repository states and the flag off; assert the harness builds no state, makes no call, and constructs no `Http_Client` path; assert the fake records zero calls; minimum 100 iterations
    - **Validates: Requirements 1.4, 1.5, 2.4**

- [ ] 5. Aspect module: token budget guard and drift
  - [ ] 5.1 Implement the token-budget estimator and guard in `jev/mod.rs`
    - Implement `estimate_tokens(request)` that counts the serialized request bytes and divides by `BYTES_PER_TOKEN`, rounding up (conservative)
    - Implement `guard_budget(request)` that returns `BudgetExceeded` naming the estimate and the budget when the estimate is more than `TOKEN_BUDGET`, and `Ok(())` otherwise
    - Send no request on an over-budget estimate (R4.10)
    - _Requirements: 4.10, 6.5_

  - [ ]* 5.2 Write property test for the token-budget bound
    - **Property 33: Token-budget bound**
    - Drive with generated states and question sets, some over budget; assert every built request stays at or under the budget, or the harness returns `BudgetExceeded` before the call; minimum 100 iterations
    - **Validates: Requirements 4.10**

  - [ ] 5.3 Implement drift evaluation in `jev/drift.rs`
    - Define `DriftOutcome` (`noul`, `out_of_scope`, `forced_by_path_match`)
    - Run the model-free literal path match first: when a written path matches a `protected_paths` entry, return `out_of_scope = true` and `forced_by_path_match = true` with no Jev call (R5.8)
    - Otherwise build one Noul, read the `noul` value, and apply the configured drift boundary inside the harness; the threshold step carries the determinism (R5.4 to R5.6)
    - Record the `noul` value and the pass-or-fail result (R5.9)
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.8, 5.9_

  - [ ]* 5.4 Write property test for drift boundary determinism
    - **Property 25: Drift boundary determinism**
    - Drive with generated `noul` values, boundaries, and plans with protected paths; assert two evaluations of the same input return the same result, and a protected-path plan is out-of-scope through the model-free layer with no call; minimum 100 iterations
    - **Validates: Requirements 5.7, 5.8**

- [ ] 6. Aspect module: routing
  - [ ] 6.1 Implement routing evaluation in `jev/routing.rs`
    - Define `RoutingOutcome` (`target`, `confidence`, `band`)
    - Build one Choice over the Routing_Target set plus `NONE` (R3.1, R3.2)
    - Map a chosen option inside the set to exactly one member (R3.3); return `UnexpectedOption` naming the option and the expected set and record no route for an outside option (R3.4)
    - Band the confidence through `confidence_band` and record the target, the confidence, and the band (R3.5 to R3.8)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8_

  - [ ]* 6.2 Write property test for routing option closure
    - **Property 28: Routing option closure**
    - Drive the fake with Choice answers inside and outside the target set; assert an inside option maps to a member and an outside option returns `UnexpectedOption` with no route; minimum 100 iterations
    - **Validates: Requirements 3.2, 3.3, 3.4**

- [ ] 7. Aspect module: parallel rigor scoring
  - [ ] 7.1 Implement rigor evaluation in `jev/rigor.rs`
    - Define `RigorReport` (the four signals plus `latency_ms`)
    - Build one request with exactly four questions: three Nouls (hallucinated import, violates conventions, contains secrets) and one complexity Score (R4.1 to R4.5)
    - Guard the token budget before the call and reject an over-budget request (R4.10)
    - Read the four answers by caller-chosen id; return `MissingAnswer` for an absent id (R4.7) and `DuplicateAnswer` for a repeated id (R4.8), retaining no partial answer
    - Record the four values and one latency for the single request (R4.9)
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10_

  - [ ]* 7.2 Write unit tests for the rigor response validation
    - Test the missing-id `MissingAnswer` error and the duplicate-id `DuplicateAnswer` error, each retaining no partial answer
    - Test a request at exactly four questions and the over-budget rejection
    - _Requirements: 4.7, 4.8, 4.1, 4.10_

- [ ] 8. Aspect module: context pruning
  - [ ] 8.1 Implement pruning evaluation in `jev/pruning.rs`
    - Define `PruningOutcome` (`kept`, `kept_count`, `dropped_count`, `input_count`)
    - Build one Score that rates each line's relevance; split the log into ordered chunks, each at or under `TOKEN_BUDGET`, when the log exceeds the budget (R6.5)
    - Keep every line at or over the keep threshold in original order, drop the rest (R6.2 to R6.4)
    - Record the three counts, which sum to the input count (R6.6, R6.7)
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7_

  - [ ]* 8.2 Write property test for pruning line conservation
    - **Property 26: Pruning line conservation**
    - Drive the fake with generated logs and keep thresholds; assert `kept_count + dropped_count == input_count` for every case; minimum 100 iterations
    - **Validates: Requirements 6.7**

  - [ ]* 8.3 Write property test for pruning order preservation
    - **Property 27: Pruning order preservation**
    - Drive the fake with generated logs and keep thresholds; assert the kept list is an in-order subsequence of the input; minimum 100 iterations
    - **Validates: Requirements 6.4**

- [ ] 9. Aspect module: self-healing decisions
  - [ ] 9.1 Implement self-healing evaluation in `jev/self_heal.rs`
    - Define `SelfHealDecision` (`instruction`, `confidence`) and the `SelfHeal` enum (Revert, RefactorImports, SimplifyLogic, AskHuman)
    - Trigger on a rigor failure: any Noul at or over the rigor-failure boundary, or a complexity Score at or over the complexity boundary (R7.1)
    - Build one Choice over `REVERT`, `REFACTOR_IMPORTS`, `SIMPLIFY_LOGIC`, `ASK_HUMAN`; map an inside option to a typed instruction (R7.2); return `UnexpectedOption` for an outside option (R7.3)
    - Apply the safety overrides in order: a confidence under the low threshold returns `AskHuman` (R7.4); a `REVERT` under the destructive threshold returns `AskHuman` (R7.5)
    - Record the chosen instruction and the confidence (R7.6)
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

  - [ ]* 9.2 Write property test for self-healing option closure
    - **Property 29: Self-healing option closure**
    - Drive the fake with Choice answers inside and outside the fixed set; assert an inside option maps to a typed instruction and an outside option returns a typed error; minimum 100 iterations
    - **Validates: Requirements 7.2, 7.3**

  - [ ]* 9.3 Write property test for low-confidence self-healing safety
    - **Property 30: Low-confidence self-healing safety**
    - Drive the fake with `REVERT` answers at a confidence under the destructive threshold; assert the harness returns `ASK_HUMAN` and no `REVERT` instruction; minimum 100 iterations
    - **Validates: Requirements 7.5**

- [ ] 10. Checkpoint - Make sure that the trait, config, and aspect tests pass
  - Make sure that all tests pass offline against the fake with no `jev-http` feature. Ask the user if questions arise.

- [ ] 11. HTTP client behind the jev-http feature
  - [ ] 11.1 Implement the pure status mapper in `jev/client_http.rs`
    - Implement a pure function over a status code and a body that maps 200 to a parse, 401 to `Unauthorized`, 422 to `Validation` naming the field, 429 and 529 to a retry signal, and any other status to `UnexpectedStatus`
    - Compile the pure mapper always (not behind `jev-http`), so its property test runs offline (design note on P32)
    - Raise no panic and call no `unwrap` or `expect` on any branch
    - _Requirements: 10.1, 10.2, 10.6, 10.7_

  - [ ]* 11.2 Write property test for error non-panic
    - **Property 32: Error non-panic**
    - Drive the pure mapper with statuses 401, 422, 429, 529, and an unmapped status; assert each returns a typed error and raises no panic; run offline with no `jev-http` feature; minimum 100 iterations
    - **Validates: Requirements 10.6, 10.7**

  - [ ] 11.3 Implement the Http_Client transport behind `jev-http`
    - Define `Http_Client` as a named type that satisfies `Jev_Client`, gated behind the `jev-http` feature
    - Read the API key from `TRUENORTH_JEV_API_KEY` at call time; return `MissingApiKey` naming the variable and the remediation and make no call when absent (R2.6, R9.4)
    - Send the `Authorization` bearer header and the `Content-Type: application/json` header, and set `model` to `jev-latest` (R2.9, R2.10)
    - Apply a 30-second wall-clock timeout; return `Timeout` on expiry and `Network` on a pre-response failure (R2.5, R2.8)
    - Implement `retry_with_backoff` that retries 429 and 529 with exponential backoff up to `retry_limit`, then returns `RateLimitedOrOverloaded` naming the last status and the attempt count (R10.3 to R10.5)
    - Validate the response through the shared parse: reject a missing id (`MissingAnswer`) and a duplicate id (`DuplicateAnswer`), retaining no partial result (R4.7, R4.8, R2.8)
    - Never write, echo, or log the API key (R9.5, R9.6)
    - _Requirements: 2.3, 2.5, 2.6, 2.8, 2.9, 2.10, 9.4, 9.5, 9.6, 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7_

  - [ ]* 11.4 Write feature-gated example tests for the transport
    - Gate behind `jev-http`; test the missing-key `MissingApiKey` naming `TRUENORTH_JEV_API_KEY`, the 401 `Unauthorized`, the 422 `Validation` naming the field, and a 429 sequence that retries to the limit then returns `RateLimitedOrOverloaded`
    - Drive with a named fake transport, so the test makes no real network call
    - _Requirements: 2.6, 10.1, 10.2, 10.3, 10.5_

- [ ] 12. Benchmark runner and bin
  - [ ] 12.1 Implement the benchmark types and runner in `jev/bench.rs`
    - Define `AspectMetrics`, `CostEstimate`, `Fixture`, and `ExpectedAnswers`
    - Load a labeled fixture set that pairs each real repository case with an expected answer per aspect; return `InvalidFixtureSet` before any aspect runs and retain any prior report on a missing or unparsable set (R11.8, R11.9)
    - Exercise the five aspects against the fixtures and select the fake or the real client by config (R11.1, R11.2)
    - Report per-invocation latency, per-case confidence where an aspect returns one, and the agreement fraction (R11.3 to R11.5)
    - Account for input tokens and output tokens separately; report an estimated cost only with a configured price and label it unverified; report token counts and no cost with no configured price (R12.1, R12.2, R12.6, R12.7)
    - Write the report under `.agent/telemetry/` through `write_under_agent`, excluding the API key and any denylist content (R11.6)
    - Hardcode no per-token price; read the price only from config (R12.8)
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.8, 11.9, 12.1, 12.2, 12.6, 12.7, 12.8_

  - [ ] 12.2 Implement the benchmark bin at `runtime/src/bin/jev_bench.rs`
    - Gate the bin behind the `jev-http` feature and call `engine::jev::bench`
    - Carry no `#[test]`, so `cargo test` does not run the bin (R11.10)
    - Select the fake or the real client by config; make no network call under the fake (R11.2, R11.7)
    - _Requirements: 11.2, 11.7, 11.10_

  - [ ]* 12.3 Write unit tests for the benchmark accounting
    - Test the agreement fraction over a labeled fixture set and the missing-fixture `InvalidFixtureSet` that runs no aspect and retains any prior report
    - Test that input and output tokens track separately, a zero output rate yields a zero output cost, and an absent price reports counts and no cost
    - Test that the report under `.agent/telemetry/` carries no denylist content and no key
    - _Requirements: 11.5, 11.9, 12.1, 12.2, 12.6, 11.6_

- [ ] 13. Structural and smoke checks
  - [ ]* 13.1 Write structural smoke checks
    - Assert the benchmark bin carries no `#[test]`, so `cargo test` does not run it (R11.10)
    - Assert no per-token price literal appears in an acceptance-tested code path; the price comes only from config (R12.8)
    - Assert both `Fake_Client` and `Http_Client` satisfy `Jev_Client` (compilation)
    - _Requirements: 11.10, 12.8, 2.1, 2.2, 2.3_

- [ ] 14. Final checkpoint - Full verification across both builds
  - Run `cargo fmt` and `cargo clippy` with warnings denied on the default build and on `--features jev-http`
  - Run `cargo test` offline and confirm the default build compiles no reqwest
  - Make sure that all tests pass across both builds. Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional (test tasks) and can be skipped for a faster MVP. Core implementation tasks are never optional.
- The harness is engine infrastructure, not an MCP tool. It registers nothing into the tool router, no gate, and no resource.
- Two layers gate the harness: the runtime `jev` flag (default off) and the Cargo `jev-http` feature. The default build and `cargo test` compile no HTTP client (P22).
- The pure status mapper (task 11.1) is always compiled, so P32 runs offline. The transport (task 11.3) and the bench bin (task 12.2) compile only under `jev-http`.
- Each task references specific requirement clauses for traceability. Every property P22 to P33 maps to one property test at a minimum of 100 iterations, driven by the named `Fake_Client`.
- Property to task map: P22 (4.2), P23 (3.4), P24 (3.5), P25 (5.4), P26 (8.2), P27 (8.3), P28 (6.2), P29 (9.2), P30 (9.3), P31 (3.7), P32 (11.2), P33 (5.2).

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "1.2"] },
    { "id": 1, "tasks": ["1.3", "2.1"] },
    { "id": 2, "tasks": ["2.2", "2.3"] },
    { "id": 3, "tasks": ["3.1", "3.3", "3.6"] },
    { "id": 4, "tasks": ["3.2", "3.4", "3.5", "3.7", "4.1", "5.1"] },
    { "id": 5, "tasks": ["4.2", "5.2", "5.3", "6.1", "7.1", "8.1", "9.1"] },
    { "id": 6, "tasks": ["5.4", "6.2", "7.2", "8.2", "8.3", "9.2", "9.3", "11.1"] },
    { "id": 7, "tasks": ["11.2", "11.3", "12.1"] },
    { "id": 8, "tasks": ["11.4", "12.2"] },
    { "id": 9, "tasks": ["12.3", "13.1"] }
  ]
}
```
