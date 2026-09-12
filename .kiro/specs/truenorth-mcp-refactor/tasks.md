# Implementation Plan: TrueNorth-MCP Refactor

## Overview

This plan implements the Rust rewrite of the passive `bigpowers-mcp` TypeScript catalog
server into TrueNorth-MCP, an active, protocol-first MCP execution runtime, plus
npm/pnpm binary distribution and a parity-gated repository cleanup.

The runtime crate is written in **Rust** (rmcp, serde, serde_yaml, schemars, tokio,
notify, regex, tree-sitter optional). The npm wrapper is **JavaScript/Node.js**. These
languages are fixed by the design (Part II §1 module layout, §7 distribution). No
pseudocode language selection is required.

Tasks build incrementally. The engine layer is implemented and tested before the tool and
resource layers that consume it. Tools and resources are wired into the stdio server, and
distribution follows. Per the design's **parity-before-removal** rule (§9.3, Req 10.5),
the destructive repository cleanup is sequenced last and gated on the crate and npm wrapper
passing their parity suites.

Property-based / golden tests target the five design correctness properties:
P1 (ontology gate), P2 (verify gate), P3 (state preservation), P4 (tier transforms),
P5 (no dangling references).

## Tasks

- [x] 1. Rust crate scaffold and stdio server boot
  - Create `Cargo.toml` with dependencies: rmcp, serde, serde_yaml, schemars, tokio, notify, regex, anyhow, and tree-sitter as an optional feature
  - Create the module skeleton: `src/index.rs`, `src/config.rs`, `src/resources/mod.rs`, `src/tools/mod.rs`, `src/engine/mod.rs` (with empty submodule stubs for `spec`, `validate`, `gate_runner`, `tier`, `watcher`, `git`, `ontology_scan`)
  - Implement `src/index.rs` entrypoint that builds the server over the rmcp `StdioServerTransport` and responds to the MCP `initialize` handshake
  - If the stdio transport fails to initialize, make sure that startup terminates with a non-zero exit status and an error indication
  - _Requirements: 1.1, 1.2, 1.3_
  - Done: merged to `main` in PR #1. Notes for later tasks:
    - The crate lives in a new `truenorth-mcp/` directory. The old `bigpowers-mcp/` stays until parity (task 21).
    - Pinned rmcp 3.3 and schemars 1, edition 2024 (the design cited rmcp 0.16 and edition 2021 as illustrative only).
    - The entrypoint is `src/main.rs`, not `src/index.rs`.
    - The `#[tool_router]` and `#[tool_handler]` macros need at least one `#[tool]` method, so they arrive with the first tool (task 8).
    - Verified: `cargo build`, `cargo fmt --check`, `cargo clippy -D warnings` clean; a real `initialize` request returns server info; a closed transport returns a non-zero exit.

- [x] 2. Engine config: root resolution, denylist, git scope, sandbox config
  - [x] 2.1 Implement `engine::config` (`config.rs`)
    - `get_repo_root()` evaluates candidates in order: `TRUENORTH_ROOT` env (when set and non-empty), then cwd, then parent-of-package, selecting the first that directly contains both `skills/` and `specs/`
    - Terminate with a non-zero exit status and a "no valid repository root" error when no candidate satisfies the rule
    - `secret_denylist()` covering `.env`, `*.pem`, and any path containing a `secret` or `credentials` marker
    - `GIT_SCOPE_DIRS = ["skills", "specs"]` constant and `MAX_READ_SKILL_BYTES`
    - `SandboxConfig { timeout, working_dir, allowlist, execution_enabled }`
    - _Requirements: 1.4, 1.5, 1.6, 1.7, 1.8_

  - [x]\* 2.2 Write unit tests for config
    - Test root resolution precedence (env over cwd over parent) and marker-directory validation
    - Test the no-valid-root error path
    - Test denylist matching for `.env`, `*.pem`, `secret`, and `credentials` paths
    - _Requirements: 1.4, 1.5, 1.6, 1.7_

- [x] 3. Engine spec models and backward-compat validation
  - [x] 3.1 Implement `engine::spec` serde models
    - Model `state.yaml` (`active_epic`, `active_story`, `handoff.{next_skill,context,epic}`, `metrics.skill_timings`, `release.*`, `git.branch`) and `release-plan.yaml` (`release.*`, `build_order[]`, `done_epics_summary`) with a `#[serde(flatten)]` catch-all preserving unknown fields
    - Preserve a `bigpowers_version` key with its original value on read and write
    - Model `ontology.yaml` (`Ontology`, `Entity`, `Constraint`) per §3.2 with `schemars` derives
    - _Requirements: 9.1, 9.3, 9.4, 4.3_

  - [x] 3.2 Implement `engine::validate`
    - Validate `state.yaml` / `release-plan.yaml` reads and writes against the observed bigpowers schemas
    - Reject reads that fail validation with an error identifying the file and the failed constraint, leaving the file unmodified
    - Map legacy phase names: `Build→Execute`, `Verify→Review`, `Release/Sustain→Integrate`. Reject unrecognized legacy phase names with an identifying error
    - _Requirements: 9.1, 9.2, 9.5, 9.6_

  - [x]\* 3.3 Write property test for state preservation
    - **Property 3: State files stay schema-valid after any mutation**
    - Round-trip and mutate real `specs/*.yaml` fixtures. Assert every unknown field (including `bigpowers_version`) is preserved with original key and value and the post-state still validates
    - **Validates: Requirements 9.3, 9.4**

  - [x]\* 3.4 Write unit tests for validation and phase mapping
    - Test validation-failure read rejection and unchanged-file guarantee
    - Test legacy phase-name mapping and the unrecognized-phase rejection
    - Test ontology model round-trip against a fixture
    - _Requirements: 9.1, 9.2, 9.5, 9.6, 4.3_

- [x] 4. Engine tier transforms
  - [x] 4.1 Implement `engine::tier`
    - `render_skill(md, tier)` with `Full` = byte-for-byte identity
    - `strip_meta_steps` (reasoning): remove meta/guardrail scaffolding and Anthropic-style XML wrappers while retaining every heading and invariant/acceptance-criterion statement
    - `compress_for_local_context` (lean): drop rationale/background/verbose-example sections, convert headings to imperative bullets, dedupe directives, and truncate to the configured lean token budget while retaining every invariant/acceptance-criterion statement
    - _Requirements: 6.6, 6.7, 6.8_

  - [x]\* 4.2 Write property/golden test for tier transforms
    - **Property 4: Tier transforms preserve invariants**
    - Golden fixtures assert `full == md` identity. Assert reasoning/lean retain invariant and acceptance-criterion statements and remove only meta/guardrail (and, for lean, rationale/background/verbose examples within budget)
    - **Validates: Requirements 6.6, 6.7, 6.8**

- [x] 5. Engine gate runner (sandbox)
  - [x] 5.1 Implement `engine::gate_runner`
    - `run_gate(cmd, cfg) -> GateOutcome` per §5: reject when execution disabled (evidence-only), allowlist check on the first token, spawn with `cwd` pinned under repo root and sanitized environment (drop denylist-matching values)
    - Wall-clock timeout (default 300s) with hard kill. Return `{passed:true}` only on exit code 0 within timeout. On non-zero exit, return an error with at most the final 2 KB of stderr plus remediation hints. On timeout, return a timeout error with reduce-scope/raise-timeout hints. On allowlist miss, return an error without executing
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

  - [x]\* 5.2 Write property/unit tests for the gate runner
    - **Property 2: verify_gate passes only on exit 0**
    - Using a fake command runner, cover pass (exit 0), fail (non-zero, stderr tail at most 2 KB), timeout (hard kill), allowlist-reject (no execution), and evidence-only paths
    - Assert `cwd` pinning under repo root and environment sanitization of denylist-matching values
    - **Validates: Requirements 3.2, 3.3, 3.4, 3.5, 3.6**

- [x] 6. Engine ontology scan
  - [x] 6.1 Implement `engine::ontology_scan` regex baseline
    - `OntologyAnalyzer` trait and `Violation { constraint_id, path, line, message }`
    - `RegexAnalyzer` (language-agnostic baseline, always available) detects prohibited aliases as identifiers and emits violations citing the owning constraint id with a remediation hint naming the correct term (for example, `is_deleted` maps to C-02)
    - Define the AST-analyzer plug-in seam (`pick_analyzer` selecting AST where a grammar exists, else regex baseline)
    - _Requirements: 4.5, 4.6, 4.9_

  - [x]\* 6.2 Write property test for the ontology gate
    - **Property 1: Ontology gate rejects prohibited aliases**
    - Assert every prohibited alias that occurs as an identifier yields at least one violation citing the owning constraint id. Include the `is_deleted` maps to C-02 case with the exact remediation string
    - **Validates: Requirements 4.5, 4.6**

  - [ ]* 6.3 Implement the tree-sitter AST analyzer (optional)
    - `AstAnalyzer` behind the tree-sitter feature for supported languages, plugged in via `pick_analyzer`, adding precision without blocking the regex baseline
    - _Requirements: 4.9_

- [x] 7. Engine git context and file watcher
  - [x] 7.1 Implement `engine::git`
    - Git `status` / `log` / `diff` scoped to `GIT_SCOPE_DIRS` (`skills/`, `specs/`), excluding changes outside those directories
    - `git_changed_files_in_scope()` helper for ontology default scope
    - _Requirements: 1.8, 4.7_

  - [x] 7.2 Implement `engine::watcher`
    - notify-based file watch that debounces edits within a 200 ms window and coalesces them, emitting `notifications/resources/updated` for the affected resource URI within 1 second of detecting a change
    - _Requirements: 5.3, 5.6_

  - [x]\* 7.3 Write unit tests for git scope and watcher debounce
    - Assert git output excludes paths outside `skills/`/`specs/`
    - Assert two edits within the debounce window coalesce into a single notification
    - _Requirements: 1.8, 5.6_

- [ ] 8. Skills tools: tiered get_skill and ported legacy catalog
  - [x] 8.1 Implement `get_skill` (tiered) in `tools/skills.rs`
    - Require `name`, accept optional `tier`. Resolve effective tier from the per-call arg, else `TRUENORTH_TIER`, else `full`
    - Reject unrecognized `tier` values and unresolved `name` without returning a payload. Apply `engine::tier` rendering
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

  - [ ] 8.2 Port the legacy catalog tools in `tools/skills.rs`
    - `index_skills`, `read_skill`, `search_skills`, `build_skill_graph`, `read_graph`, `search_nodes`, `open_nodes`, `get_dependencies`, `get_git_context`, `validate_skill`, exposed concurrently with the active tools
    - `read_skill` parses frontmatter/headings/sections and errors on missing/unresolved/parse-failure. `search_skills` uses case-insensitive substring (whole-string when `exact`) with empty-query error and empty-result set. `get_git_context` action status|log|diff (default status) with unsupported-action error. `build_skill_graph` builds and persists the graph queried by `read_graph`/`search_nodes`/`open_nodes`, which error when no graph is persisted
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9_

  - [ ]* 8.3 Write unit tests for skills and legacy catalog tools
    - Test tier resolution precedence and both error paths of `get_skill`
    - Test `read_skill`, `search_skills`, `get_git_context`, and graph tools including their error paths
    - _Requirements: 6.2, 6.3, 6.4, 6.5, 7.3, 7.4, 7.5, 7.7, 7.9_

- [ ] 9. Lifecycle tools: advance_phase and record_task
  - [ ] 9.1 Implement `tools/lifecycle.rs`
    - `truenorth_advance_phase` (schemars contract: `from_phase`, `to_phase` each one of the six phases, `artifacts_summary` 1–4000 chars). On success, write `state.yaml` and record git-scoped context, emit `resources/updated` for `truenorth://state`
    - `truenorth_record_task` (schemars contract: `epic_id` matching `^e[0-9]+([a-z0-9-]*)?$`, `task_name` 1–200 chars, `verify_command` 1–1000 chars). On success, append to `release-plan.yaml`, emit `resources/updated` for `truenorth://cockpit`
    - Reject schema-violating input identifying the offending field with no partial mutation. On write failure, report it and leave the target file in its pre-invocation state
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.11, 2.12_

  - [ ]* 9.2 Write unit tests for lifecycle tools
    - Test schema validation rejection (missing/invalid fields) with no partial mutation and the write-failure pre-state guarantee
    - Test successful `state.yaml` write and `release-plan.yaml` append
    - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.11, 2.12_

- [ ] 10. Gate tool: verify_gate
  - [ ] 10.1 Implement `tools/gates.rs`
    - `truenorth_verify_gate` (schemars contract: required `phase`, optional `test_evidence`, `mode` execute|evidence default execute) delegating to `engine::gate_runner`
    - Default execute mode runs the project verify/test command in the sandbox. Evidence mode skips execution and requires `test_evidence`, and rejects the call when it is absent
    - _Requirements: 3.1, 3.7, 3.8_

  - [ ]* 10.2 Write unit tests for verify_gate
    - Test default-execute delegation, evidence-mode opt-out, and the missing-evidence rejection
    - _Requirements: 3.1, 3.7, 3.8_

- [ ] 11. TDD tool: tdd_cycle
  - [ ] 11.1 Implement `tools/tdd.rs`
    - `truenorth_tdd_cycle` (schemars contract: `step` red|green|refactor, `failing_test_cmd` 1–1000 chars, `files_to_modify` 1–100 non-empty entries)
    - Enforce Red then Green then Refactor ordering. Reject out-of-order transitions with an invalid-transition error that leaves recorded TDD state unchanged
    - Red-stage semantics: report red passed when the failing test command exits non-zero. Report red failed with a "test did not fail as required" error when it exits 0
    - _Requirements: 2.1, 2.6, 2.7, 2.8, 2.9, 2.10, 2.11_

  - [ ]* 11.2 Write unit tests for tdd_cycle
    - Test ordering enforcement, invalid-transition rejection with unchanged state, and both red-stage exit-code outcomes
    - _Requirements: 2.6, 2.7, 2.8, 2.9, 2.10_

- [ ] 12. Ontology tools: generate_ontology and verify_ontology
  - [ ] 12.1 Implement `tools/ontology.rs`
    - `truenorth_generate_ontology` (schemars contract: `domain` 1–200 chars, `source_paths` at least 1 entry). On success, write `specs/ontology.yaml` with entities (invariants, states, transitions, prohibited aliases) and global constraints. Reject invalid input identifying the parameter without writing. On write failure, leave any existing file unchanged. Seed as a new file only, and error when `specs/ontology.yaml` already exists
    - `truenorth_verify_ontology` (schemars contract: optional `scope_paths`) delegates to `engine::ontology_scan`. Default scope to git-changed files in scope. Return passed with zero files scanned when scope is empty. Return passed when no violations. Otherwise return the formatted first-violation error
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.7, 4.8, 4.10, 9.9, 9.10_

  - [ ]* 12.2 Write unit tests for ontology tools
    - Test generate invalid-input, write-failure, and already-exists error paths. Test verify default-scope, empty-scope pass, and no-violation pass
    - _Requirements: 4.1, 4.2, 4.4, 4.7, 4.8, 4.10, 9.9, 9.10_

- [ ] 13. Checkpoint - Make sure that all engine and tool tests pass
  - Make sure that all tests pass. Ask the user if questions arise.

- [ ] 14. Resources layer: cockpit and ontology resources
  - [ ] 14.1 Implement `resources/cockpit.rs` and `resources/ontology.rs`
    - Serve `truenorth://state` (backed by `state.yaml`), `truenorth://cockpit` (release plan + product boundary), `truenorth://conventions` (coding standards), and `truenorth://ontology` (`ontology.yaml`)
    - Read current on-disk content on `resources/read`, treating disk as source of truth. Support `resources/list`, `resources/read`, and subscribe
    - _Requirements: 5.1, 5.2, 5.5_

  - [ ] 14.2 Wire resource change notifications
    - Emit `notifications/resources/updated` on watcher-detected disk change and on tool-driven write. On YAML parse/schema-validation failure, return a resource read error identifying the file and failure, retain the last successfully parsed content, and continue serving other resources without terminating
    - _Requirements: 5.3, 5.4, 5.7_

  - [ ]* 14.3 Write unit tests for resources
    - Test on-disk-read source-of-truth behavior and the parse-failure read-error (non-crash, last-good retention) path
    - _Requirements: 5.5, 5.7_

- [ ] 15. Wire the server and drive an end-to-end lifecycle
  - [ ] 15.1 Aggregate tools and resources in `src/index.rs`
    - Assemble the `#[tool_router]` (active + legacy tools) and `ServerHandler` (resources), spawn the watcher, and serve over stdio
    - _Requirements: 1.1, 2.1, 5.1_

  - [ ]* 15.2 Write integration test for the full lifecycle
    - In-process MCP client drives `resources/list` + `resources/read` and a full Discover to Integrate tool sequence against a temp repo. Assert `resources/updated` fires after a disk edit (watcher) and after a tool write
    - _Requirements: 1.1, 2.1, 2.3, 5.3, 5.4, 11.1_

- [ ] 16. Model and harness agnosticism verification
  - [ ] 16.1 Confirm agnostic delivery in emitted payloads
    - Make sure that emitted skills/instructions contain no Anthropic-specific XML tags or vendor-directed meta-instruction scaffolding. Apply language-agnostic gate/ontology processing with no per-language configuration. When the requesting model family or harness cannot be identified, deliver default language-agnostic content and indicate no model/harness adaptation was applied
    - _Requirements: 11.1, 11.4, 11.5_

  - [ ]* 16.2 Write tests for agnostic content identity
    - Assert no Anthropic XML/meta scaffolding in emitted payloads. Assert identical content across simulated model families and harnesses. Assert the unidentified-client default path indicates no adaptation
    - _Requirements: 11.1, 11.2, 11.3, 11.5_

- [ ] 17. Checkpoint - Make sure that the runtime parity suite passes
  - Make sure that all tests pass. Ask the user if questions arise.

- [ ] 18. npm distribution wrapper
  - [ ] 18.1 Create the root and platform package manifests
    - `npm/package.json` (root `truenorth-mcp` with `bin/truenorth.js` and the four `@truenorth-mcp/<platform>` packages as optionalDependencies)
    - Platform stub `package.json` files for darwin-arm64, darwin-x64, linux-x64, linux-arm64 each declaring matching `os` and `cpu` fields, with windows-x64 left out of scope
    - _Requirements: 8.1, 8.2, 8.6, 8.8_

  - [ ] 18.2 Implement `npm/bin/truenorth.js` runner and init scaffold
    - Resolve the single platform-native binary from `process.platform`/`process.arch`, spawn with stdio passthrough, and propagate the child's exit code
    - Print an error to stderr and exit 1 on unsupported platform and on spawn failure
    - `init` scaffolds `specs/` from the crate template when none exists. When `specs/` already exists, leave it unchanged and print a skip message to stderr
    - _Requirements: 8.3, 8.4, 8.5, 8.9, 8.10_

  - [ ]* 18.3 Write wrapper resolution tests
    - Verify the runner resolves the correct `@truenorth-mcp/<platform>-<arch>` package and exits 1 with a clear message on unsupported platforms and spawn failure
    - _Requirements: 8.3, 8.4, 8.9_

- [ ] 19. CI cross-compile and publish matrix
  - [ ] 19.1 Add the release GitHub Actions workflow
    - On release tags, cross-compile `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, and `aarch64-unknown-linux-gnu`. Package each per-platform binary and publish per-platform packages. Publish the root wrapper only after all per-platform packages publish successfully. If any per-platform publish fails, abort the release and skip the root wrapper
    - _Requirements: 8.7, 8.11_

- [ ] 20. Checkpoint - Confirm crate + npm wrapper parity before cleanup
  - Make sure that the full parity test suites (crate + npm wrapper) pass with zero failures. Ask the user if questions arise. Do not proceed to cleanup unless parity holds (Req 10.5, §9.3).

- [ ] 21. Repository cleanup (parity-gated, one commit per batch)
  - [ ] 21.1 Consolidate the 13 per-harness skill mirror directories into one `skills/`
    - Fold `.cline/ .codebuddy/ .codex/ .continue/ .copilot/ .cursor/ .gemini/ .kilocode/ .opencode/ .pi/ .qwen/ .trae/ .windsurf/` skill mirrors into a single canonical `skills/`. Assert exactly one `skills/` remains and zero mirror directories remain. Commit as its own batch
    - _Requirements: 10.1, 10.6_

  - [ ] 21.2 Remove analysis docs and vendor-coupled agent files
    - Remove the upstream one-off analysis docs and `CLAUDE.md`, `GEMINI.md`, `opencode.json`, `.mcp.json`. Commit as its own batch
    - _Requirements: 10.2, 10.6_

  - [ ] 21.3 Remove the bash/python script pipeline and upstream infra directories
    - Remove the `scripts/` pipeline and the `kernel/`, `profiles/`, `extensions/`, `hooks/`, `dashboard/`, and `website/` directories. Commit as its own batch
    - _Requirements: 10.2, 10.6_

  - [ ] 21.4 Remove superseded entrypoints and generated artifacts
    - Remove `bin/*.js`, `index.js`, `requirements.txt`, the auto-generated skill index and lock files, upstream `templates/`, and the regenerable `specs/` process artifacts, JSON side-cars, and wiki directories. Commit as its own batch
    - _Requirements: 10.3, 10.6_

  - [ ] 21.5 Gitignore allure-results and run the dangling-reference check
    - Add a `.gitignore` entry matching `allure-results/` such that `git status` reports zero tracked/untracked files under it
    - **Property 5: No dangling references after cleanup**. Assert every path referenced by a retained skill or by the runtime (config, resource URIs, cockpit reads) still resolves on disk with zero unresolved references
    - Commit as its own batch
    - _Requirements: 10.4, 10.7_

- [ ] 22. Final checkpoint - Make sure that all tests pass and no dangling references remain
  - Make sure that all tests pass. Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional (test and nice-to-have tasks) and can be skipped for a faster MVP. Core implementation and cleanup tasks are never optional.
- The tree-sitter AST analyzer (6.3) is optional beyond the regex baseline. Windows-x64 is out of scope per Req 8.8.
- Each task references specific requirement clauses for traceability.
- Checkpoints give incremental validation. Task 20 is the parity gate that must hold before the destructive cleanup in task 21 begins (parity-before-removal, Req 10.5 / §9.3).
- Property tests validate the five design correctness properties: P1 (6.2), P2 (5.2), P3 (3.3), P4 (4.2), P5 (21.5).
- Each cleanup removal batch is its own git commit so it is individually revertable (Req 10.6).

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1"] },
    { "id": 1, "tasks": ["2.1"] },
    { "id": 2, "tasks": ["2.2", "3.1", "4.1", "5.1", "7.1"] },
    { "id": 3, "tasks": ["3.2", "4.2", "5.2", "6.1", "7.2"] },
    { "id": 4, "tasks": ["3.3", "3.4", "6.2", "6.3", "7.3", "8.1", "8.2"] },
    { "id": 5, "tasks": ["8.3", "9.1", "10.1", "11.1", "12.1"] },
    { "id": 6, "tasks": ["9.2", "10.2", "11.2", "12.2", "14.1"] },
    { "id": 7, "tasks": ["14.2", "16.1"] },
    { "id": 8, "tasks": ["14.3", "15.1", "16.2"] },
    { "id": 9, "tasks": ["15.2", "18.1"] },
    { "id": 10, "tasks": ["18.2", "19.1"] },
    { "id": 11, "tasks": ["18.3"] },
    { "id": 12, "tasks": ["21.1"] },
    { "id": 13, "tasks": ["21.2"] },
    { "id": 14, "tasks": ["21.3"] },
    { "id": 15, "tasks": ["21.4"] },
    { "id": 16, "tasks": ["21.5"] }
  ]
}
```
