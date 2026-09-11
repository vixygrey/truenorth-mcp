# Requirements Document

## Introduction

TrueNorth-MCP is a fork of `danielvm-git/bigpowers` that rebrands and re-architects the passive `bigpowers-mcp` TypeScript catalog server into an active, protocol-first Rust MCP execution runtime. The runtime exposes spec-driven engineering discipline to AI agents as live MCP resources and active tool contracts: strict JSON-Schema calls that advance lifecycle phases, record tasks, run quality gates locally in a sandbox, enforce a shared domain ontology, and drive a Red-Green-Refactor TDD loop.

The refactor pursues three shifts while remaining backward-compatible with existing `specs/` cockpits: passive-to-active tools, cat/grep-to-resources, and Anthropic-coupled-to-agnostic skill delivery. The server is written in Rust; the projects it governs may be written in any language, so all target-code analysis is language-agnostic by default. The runtime preserves the upstream six-phase lifecycle (Discover, Design, Plan, Execute, Review & Harden, Integrate).

These requirements are derived from the approved design document (`design.md`). They cover the Rust runtime, active and legacy tool contracts, quality-gate execution, ontology generation and enforcement, the live resource cockpit, tiered skill payloads, npm/pnpm binary distribution, backward compatibility and migration, repository cleanup, and model/harness agnosticism. Design correctness properties P1 through P5 map onto acceptance criteria below so later property annotations can reference specific requirement clauses.

## Glossary

- **MCP tool**: A callable, active contract exposed by the server over the Model Context Protocol with a strict JSON Schema input, invoked by an agent to perform work (advance a phase, record a task, run a gate, transform a skill).
- **MCP resource**: A read-addressable artifact served by the server under a `truenorth://` URI (for example `truenorth://state`) that agents fetch via `resources/read` and that emits `notifications/resources/updated` when its backing file changes.
- **Cockpit**: The set of load-bearing `specs/` files the runtime reads and writes — `state.yaml`, `release-plan.yaml`, `execution-status.yaml`, `ontology.yaml`, and the `product/` and `adr/` directories — representing the live spec-driven state of a governed project.
- **Tier**: A rendering level for a skill payload — `full`, `reasoning`, or `lean` — that controls how much scaffolding is retained when a skill is returned to a model.
- **Ontology**: The domain model stored in `specs/ontology.yaml`, containing entities (with invariants, states, transitions, and prohibited aliases) and global constraints, used to detect and reject lexical and semantic drift.
- **Prohibited alias**: A forbidden synonym for an ontology entity or concept whose appearance as an identifier in scanned code signals lexical drift and constitutes an ontology violation.
- **Quality gate**: A checkpoint that confirms a phase's quality bar is met before the lifecycle proceeds.
- **Verify gate**: The specific quality gate implemented by `truenorth_verify_gate`, which by default runs the project's verify/test command in a sandbox and passes only on a real exit-0 observation, with an evidence-only opt-out mode.
- **TDD cycle**: The enforced Red-Green-Refactor step sequence driven by `truenorth_tdd_cycle`.
- **Sandbox**: The bounded subprocess execution environment used by gate runs, constrained by a wall-clock timeout, a working directory scoped under the repo root, a command allowlist, and a sanitized environment.
- **Parity-before-removal**: The cleanup sequencing rule that destructive removals backing live behaviour occur only after the Rust crate and npm wrapper reach functional parity with the legacy machinery.
- **Syncing peer**: The server's role relative to disk — it reads live, writes through, and emits notifications, but is not the sole writer; humans and other tools co-edit `specs/` and `skills/` and disk remains the source of truth.

## Requirements

### Requirement 1: Rust MCP Server Runtime

**User Story:** As an AI agent operator, I want a compiled Rust MCP server that runs over stdio and resolves the governed repository correctly, so that spec-driven discipline is delivered by a single fast binary without a Node runtime dependency at execution time.

#### Acceptance Criteria

1. THE TrueNorth_Server SHALL expose its MCP interface over standard input and output using the rmcp stdio transport.
2. IF the rmcp stdio transport fails to initialize at startup, THEN THE TrueNorth_Server SHALL terminate with a non-zero exit status and emit an error indication describing the transport initialization failure.
3. THE TrueNorth_Server SHALL replace the legacy TypeScript bigpowers-mcp catalog server as the MCP execution runtime, requiring no Node runtime dependency at execution time.
4. WHEN resolving the repository root, THE TrueNorth_Server SHALL evaluate candidate directories in the following order and select the first candidate that satisfies the repository root validation rule: (1) the directory named by the `TRUENORTH_ROOT` environment variable when that variable is set and non-empty, (2) the current working directory, (3) the parent of the package directory.
5. THE TrueNorth_Server SHALL treat a candidate directory as a valid repository root only when that directory directly contains both a `skills/` directory and a `specs/` directory.
6. IF no candidate directory satisfies the repository root validation rule, THEN THE TrueNorth_Server SHALL terminate with a non-zero exit status and emit an error indication that no valid repository root was found among the evaluated candidates.
7. WHEN reading skill or cockpit content, THE TrueNorth_Server SHALL exclude every file whose path matches the secret denylist patterns, where the denylist covers environment files, PEM files, and any path containing a `secret` or `credentials` marker, such that excluded file contents are absent from the produced output.
8. WHEN producing git context, THE TrueNorth_Server SHALL scope git status, log, and diff output to only the `skills/` and `specs/` directories, excluding changes outside those two directories from the output.

### Requirement 2: Active Tool Contracts with Strict JSON Schema

**User Story:** As an AI agent, I want active tools with strict JSON-Schema inputs for lifecycle progression, task recording, gate verification, and the TDD loop, so that I perform validated work instead of interpreting markdown blobs.

#### Acceptance Criteria

1. THE TrueNorth_Server SHALL expose the active tools `truenorth_advance_phase`, `truenorth_record_task`, `truenorth_verify_gate`, and `truenorth_tdd_cycle`, each with a JSON Schema derived from its typed input contract.
2. WHEN `truenorth_advance_phase` is invoked, THE TrueNorth_Server SHALL require `from_phase`, `to_phase`, and an `artifacts_summary` of 1 to 4000 characters, where each of `from_phase` and `to_phase` is exactly one of discover, design, plan, execute, review, or integrate.
3. WHEN `truenorth_advance_phase` completes successfully, THE TrueNorth_Server SHALL write the updated lifecycle state to `state.yaml` and record the git-scoped context.
4. WHEN `truenorth_record_task` is invoked, THE TrueNorth_Server SHALL require an `epic_id` matching the pattern `^e[0-9]+([a-z0-9-]*)?$`, a `task_name` of 1 to 200 characters, and a `verify_command` of 1 to 1000 characters.
5. WHEN `truenorth_record_task` completes successfully, THE TrueNorth_Server SHALL append the task to `release-plan.yaml`.
6. WHEN `truenorth_tdd_cycle` is invoked, THE TrueNorth_Server SHALL require a `step` of exactly one of red, green, or refactor, a `failing_test_cmd` of 1 to 1000 characters, and a `files_to_modify` array of 1 to 100 non-empty entries.
7. WHILE enforcing the TDD cycle, THE TrueNorth_Server SHALL enforce the Red, then Green, then Refactor step transitions in order.
8. IF `truenorth_tdd_cycle` is invoked with a step that does not follow the current step in Red-then-Green-then-Refactor order, THEN THE TrueNorth_Server SHALL reject the call with an error indicating the invalid transition and SHALL leave the recorded TDD state unchanged.
9. WHEN a red-stage `truenorth_tdd_cycle` runs the failing test command and that command exits with a non-zero code, THE TrueNorth_Server SHALL report the red step as passed.
10. IF a red-stage `truenorth_tdd_cycle` runs the failing test command and that command exits with code 0, THEN THE TrueNorth_Server SHALL report the red step as failed with an error indicating the test did not fail as required.
11. IF a required field of an active tool input is missing or violates its schema constraint, THEN THE TrueNorth_Server SHALL reject the call with an error identifying the offending field and SHALL NOT perform any partial mutation to cockpit state files.
12. IF an active tool cannot write its target cockpit file, THEN THE TrueNorth_Server SHALL report the write failure and SHALL leave the target file in its pre-invocation state.

### Requirement 3: Quality-Gate Execution

**User Story:** As an engineering lead, I want the verify gate to actually run the project's verify/test command in a sandbox and pass only on a real exit-0 observation, so that a model cannot hallucinate a green build.

#### Acceptance Criteria

1. WHEN `truenorth_verify_gate` is invoked without an explicit mode, THE TrueNorth_Server SHALL execute the project's verify/test command locally in the sandbox using the default execute mode.
2. THE TrueNorth_Server SHALL return `{passed: true}` from `truenorth_verify_gate` in execute mode if and only if the sandboxed command terminated with exit code 0 within the configured wall-clock timeout, whose default is 300 seconds. (Property 2)
3. IF the sandboxed command exits with a non-zero code, THEN THE TrueNorth_Server SHALL return an error result containing at most the final 2 KB of the command's standard-error output and remediation hints. (Property 2)
4. IF the sandboxed command does not terminate within the configured wall-clock timeout, THEN THE TrueNorth_Server SHALL hard-kill the command and return an error result stating the gate timed out with remediation hints to reduce test scope or raise the timeout. (Property 2)
5. IF the first token of the command is not present in the command allowlist, THEN THE TrueNorth_Server SHALL return an error result stating the command is not in the allowlist with remediation hints to add it to the allowlist or use evidence mode, and SHALL NOT execute the command. (Property 2)
6. WHILE executing a gate command, THE TrueNorth_Server SHALL scope the working directory to a path under the repository root and sanitize the environment by dropping values matching the secret denylist.
7. WHERE the invocation specifies evidence mode, THE TrueNorth_Server SHALL skip local command execution and require the caller to supply `test_evidence`.
8. IF the invocation specifies evidence mode and no `test_evidence` is supplied, THEN THE TrueNorth_Server SHALL reject the call with an error indicating that `test_evidence` is required in evidence mode.

### Requirement 4: Ontology Generation and Enforcement

**User Story:** As a domain owner, I want the server to generate a domain ontology and then reject code that drifts from it with actionable errors, so that lexical and semantic drift is caught at a quality gate.

#### Acceptance Criteria

1. WHEN `truenorth_generate_ontology` is invoked, THE TrueNorth_Server SHALL require a `domain` of 1 to 200 characters and a `source_paths` array containing at least one entry.
2. IF `truenorth_generate_ontology` is invoked with a `domain` outside 1 to 200 characters or an empty `source_paths` array, THEN THE TrueNorth_Server SHALL reject the call with an error identifying the invalid parameter and SHALL NOT write `specs/ontology.yaml`.
3. WHEN `truenorth_generate_ontology` completes successfully, THE TrueNorth_Server SHALL write `specs/ontology.yaml` containing entities with their invariants, states, transitions, and prohibited aliases, and global constraints.
4. IF `truenorth_generate_ontology` cannot write `specs/ontology.yaml`, THEN THE TrueNorth_Server SHALL report the write failure and SHALL leave any existing `specs/ontology.yaml` unchanged.
5. WHEN a file is scanned during `truenorth_verify_ontology` and a prohibited alias from the ontology appears as an identifier in that file, THE Ontology_Gate SHALL return at least one violation citing the owning constraint id. (Property 1)
6. WHEN `truenorth_verify_ontology` detects a constraint violation, THE Ontology_Gate SHALL return an error message that cites the constraint id, states the violating term, and includes a remediation hint describing the correct term. (Property 1)
7. WHEN `truenorth_verify_ontology` is invoked without explicit `scope_paths`, THE TrueNorth_Server SHALL scan the git-changed files within the configured scope.
8. WHEN `truenorth_verify_ontology` runs with no files in scope, THE Ontology_Gate SHALL return a passed result reporting zero files scanned.
9. WHEN scanning a file, THE Ontology_Gate SHALL use the language-agnostic regex analyzer as the baseline and use a pluggable AST analyzer where a grammar for the file's language is available.
10. WHEN `truenorth_verify_ontology` finds no violations, THE Ontology_Gate SHALL return a passed result.

### Requirement 5: Live MCP Resources Cockpit

**User Story:** As an AI agent, I want the cockpit state served as live MCP resources with change notifications, so that I read structured, current state instead of shelling out, while humans and other tools keep editing the same files.

#### Acceptance Criteria

1. THE TrueNorth_Server SHALL serve the resources `truenorth://state`, `truenorth://cockpit`, `truenorth://conventions`, and `truenorth://ontology` through the resources layer.
2. THE TrueNorth_Server SHALL back `truenorth://state` with `state.yaml`, `truenorth://cockpit` with the release plan and product boundary, `truenorth://conventions` with the coding standards, and `truenorth://ontology` with `ontology.yaml`.
3. WHEN a resource's backing file changes on disk, THE TrueNorth_Server SHALL emit a `notifications/resources/updated` notification for the corresponding resource URI within 1 second of detecting the change.
4. WHEN a tool writes a cockpit file, THE TrueNorth_Server SHALL emit a `notifications/resources/updated` notification for the affected resource.
5. THE TrueNorth_Server SHALL read each resource's current on-disk content when serving a `resources/read`, write tool-driven changes through to disk, and allow humans and other tools to co-edit the same files, treating the on-disk file as the source of truth.
6. WHEN two or more edits to a single backing file occur within a 200 millisecond debounce window, THE TrueNorth_Server SHALL coalesce them into a single `notifications/resources/updated` notification.
7. IF a backing file fails YAML parsing or schema validation, THEN THE TrueNorth_Server SHALL return a resource read error identifying the file and the failure, retain the last successfully parsed content in memory, and continue serving other resources without terminating.

### Requirement 6: Tiered Skill Payloads

**User Story:** As an operator running different models, I want to fetch skills at full, reasoning, or lean tiers, so that I match payload verbosity to the target model's needs without losing the discipline the skill encodes.

#### Acceptance Criteria

1. WHEN `get_skill` is invoked, THE TrueNorth_Server SHALL require a `name` argument and accept an optional `tier` argument whose value is exactly one of `full`, `reasoning`, or `lean`.
2. IF `get_skill` is invoked with a `tier` argument that is not one of `full`, `reasoning`, or `lean`, THEN THE TrueNorth_Server SHALL reject the call with an error indicating the tier value is unrecognized and SHALL NOT return a skill payload.
3. IF `get_skill` is invoked with a `name` that does not resolve to an existing skill, THEN THE TrueNorth_Server SHALL return an error indicating the named skill was not found and SHALL NOT return a skill payload.
4. WHEN `get_skill` is invoked without a `tier` argument, THE TrueNorth_Server SHALL resolve the effective tier from the `TRUENORTH_TIER` environment variable, and WHEN a per-call `tier` argument is supplied, THE TrueNorth_Server SHALL use that argument as the effective tier for that call, overriding `TRUENORTH_TIER`.
5. IF neither a per-call `tier` argument nor a recognized `TRUENORTH_TIER` value is present, THEN THE TrueNorth_Server SHALL use `full` as the effective tier.
6. WHEN `get_skill` is invoked with the full tier, THE TrueNorth_Server SHALL return the skill markdown byte-for-byte identical to the on-disk SKILL.md content. (Property 4)
7. WHEN `get_skill` is invoked with the reasoning tier, THE TrueNorth_Server SHALL return payload text that retains every heading and statement encoding an invariant or acceptance criterion from the source skill and removes only meta and guardrail scaffolding sections. (Property 4)
8. WHEN `get_skill` is invoked with the lean tier, THE TrueNorth_Server SHALL return payload text that retains every statement encoding an invariant or acceptance criterion, removes meta and guardrail scaffolding along with rationale, background, and verbose example sections, and does not exceed the configured lean token budget. (Property 4)

### Requirement 7: Legacy Catalog Tools Retained

**User Story:** As an operator migrating existing agent flows, I want the legacy catalog tools kept alongside the active tools, so that current flows keep working during the transition.

#### Acceptance Criteria

1. THE TrueNorth_Server SHALL expose the legacy catalog tools `index_skills`, `read_skill`, `search_skills`, `build_skill_graph`, `read_graph`, `search_nodes`, `open_nodes`, `get_dependencies`, `get_git_context`, and `validate_skill` concurrently with the active tools, such that every listed tool is invocable while the active tools remain available.
2. WHEN `read_skill` is invoked with a `name` that resolves to an existing SKILL.md, THE TrueNorth_Server SHALL parse and return that skill's frontmatter, headings, and sections.
3. IF `read_skill` is invoked without a `name`, or with a `name` that does not resolve to an existing SKILL.md, or whose SKILL.md fails to parse, THEN THE TrueNorth_Server SHALL return an error indicating the missing argument, unresolved skill, or parse failure respectively, without partial or malformed results.
4. WHEN `search_skills` is invoked with a `query`, THE TrueNorth_Server SHALL perform a case-insensitive substring match over skill metadata when `exact` is omitted or false and a whole-string match when `exact` is true, and SHALL return the matching skills, returning an empty result set when none match.
5. IF `search_skills` is invoked without a `query` or with an empty `query`, THEN THE TrueNorth_Server SHALL return an error indicating the missing or empty query without performing a search.
6. WHEN `get_git_context` is invoked, THE TrueNorth_Server SHALL accept an `action` of exactly one of status, log, or diff, and SHALL apply status when `action` is omitted.
7. IF `get_git_context` is invoked with an `action` other than status, log, or diff, THEN THE TrueNorth_Server SHALL return an error indicating the unsupported action without performing a git operation.
8. WHEN `build_skill_graph` is invoked, THE TrueNorth_Server SHALL build and persist the entity-relation skill graph, and `read_graph`, `search_nodes`, and `open_nodes` SHALL query that persisted graph.
9. IF `read_graph`, `search_nodes`, or `open_nodes` is invoked when no skill graph has been persisted, THEN THE TrueNorth_Server SHALL return an error indicating the graph is unavailable rather than returning empty or partial graph data.

### Requirement 8: npm/pnpm Binary Distribution

**User Story:** As a user installing the server, I want a thin npm wrapper that resolves the correct native binary per platform, so that installing and running the server is a single pnpm/npx command with only the matching binary downloaded.

#### Acceptance Criteria

1. THE Distribution_Package SHALL provide a root `truenorth-mcp` package with a `bin/truenorth.js` runner and per-platform `@truenorth-mcp/<platform>` packages declared as optionalDependencies.
2. THE Distribution_Package SHALL declare per-platform packages for darwin-arm64, darwin-x64, linux-x64, and linux-arm64, each declaring matching `os` and `cpu` fields.
3. WHEN the runner starts, THE Distribution_Package SHALL resolve the single platform-native binary matching the current `process.platform` and `process.arch`, spawn it with stdio passthrough, and on child process exit propagate the child's exit code as its own exit status.
4. IF no prebuilt binary exists for the current platform and architecture, THEN THE Distribution_Package SHALL print an error to standard error naming the platform and architecture and exit with status code 1.
5. WHEN `npx truenorth-mcp init` is invoked and no `specs/` directory exists in the target working directory, THE Distribution_Package SHALL scaffold the `specs/` cockpit from the crate's template.
6. THE Distribution_Package SHALL support MCP client configuration that launches the server via `pnpm dlx truenorth-mcp`.
7. WHEN a release tag is built, THE Continuous_Integration SHALL cross-compile the targets aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu, and aarch64-unknown-linux-gnu, publish each per-platform package, and publish the root wrapper only after all per-platform packages have published successfully.
8. THE Distribution_Package SHALL treat windows-x64 as an out-of-scope open decision rather than a supported platform.
9. IF the resolved platform-native binary fails to spawn, THEN THE Distribution_Package SHALL print an error to standard error indicating the spawn failure and exit with status code 1.
10. IF `npx truenorth-mcp init` is invoked and a `specs/` directory already exists in the target working directory, THEN THE Distribution_Package SHALL leave the existing `specs/` contents unchanged and print a message to standard error indicating that scaffolding was skipped because `specs/` already exists.
11. IF any per-platform package fails to publish during a release, THEN THE Continuous_Integration SHALL abort the release without publishing the root wrapper.

### Requirement 9: Backward Compatibility and Migration

**User Story:** As a user with an existing bigpowers cockpit, I want the runtime to read and validate my current state files without dropping data, so that I can adopt TrueNorth-MCP without rewriting my specs.

#### Acceptance Criteria

1. THE TrueNorth_Server SHALL read and validate existing `state.yaml` and `release-plan.yaml` files against the observed bigpowers schemas.
2. IF an existing `state.yaml` or `release-plan.yaml` file fails validation against the observed bigpowers schemas, THEN THE TrueNorth_Server SHALL reject the read operation, return an error indicating which file and which schema constraint failed, and leave the file unmodified on disk.
3. WHEN a tool writes a cockpit file, THE TrueNorth_Server SHALL produce a post-state that still validates against the existing bigpowers schemas and SHALL preserve every unknown field present in the pre-state with its original key and value unchanged. (Property 3)
4. WHERE a cockpit file contains a `bigpowers_version` key, THE TrueNorth_Server SHALL preserve that key with its original value during both read and write operations.
5. WHEN a legacy phase name is encountered, THE TrueNorth_Server SHALL map Build to Execute, Verify to Review, and Release and Sustain to Integrate.
6. IF a legacy phase name is encountered that is not one of Build, Verify, Release, or Sustain, THEN THE TrueNorth_Server SHALL reject the operation and return an error identifying the unrecognized phase name.
7. THE TrueNorth_Server SHALL serve skills and resources dynamically at runtime so that the `sync-skills.sh` build step is retired.
8. THE Migration_Path SHALL allow an existing `specs/` cockpit to operate through validated reads without requiring changes to its files.
9. WHEN `truenorth_generate_ontology` is invoked and `specs/ontology.yaml` does not already exist, THE Migration_Path SHALL seed `specs/ontology.yaml` as a new file.
10. IF `truenorth_generate_ontology` is invoked and `specs/ontology.yaml` already exists, THEN THE Migration_Path SHALL leave the existing file unmodified and return an error indicating the file already exists.
11. WHILE a migration is in progress, THE Migration_Path SHALL keep legacy catalog tools answering existing flows.

### Requirement 10: Repository Structure and Cleanup

**User Story:** As a maintainer, I want the upstream process exhaust consolidated or removed after parity, so that the repository reflects the lean MCP-first Rust runtime without breaking live references.

#### Acceptance Criteria

1. THE Repository SHALL consolidate the 13 per-harness skill mirror directories into a single canonical `skills/` directory such that exactly one `skills/` directory remains and zero per-harness mirror directories remain after consolidation.
2. THE Repository SHALL remove the upstream one-off analysis documents, the vendor-coupled agent files `CLAUDE.md`, `GEMINI.md`, `opencode.json`, and `.mcp.json`, the bash and python script pipeline, and the `kernel/`, `profiles/`, `extensions/`, `hooks/`, `dashboard/`, and `website/` directories such that none of the enumerated files or directories resolve on disk after removal.
3. THE Repository SHALL remove the superseded `bin/*.js` entrypoints, `index.js`, `requirements.txt`, the auto-generated skill index and lock files, the upstream templates, and the regenerable `specs/` process artifacts and side-car reports such that none of the enumerated files resolve on disk after removal.
4. THE Repository SHALL include a `.gitignore` entry matching `allure-results/` such that `git status` reports zero tracked or untracked files under `allure-results/`.
5. IF the legacy TypeScript server or any skill-referenced script backs live behaviour, THEN THE Repository SHALL retain that file until both the Rust crate and the npm wrapper pass their full parity test suites with zero failures, and SHALL remove it only after that condition holds.
6. WHEN performing cleanup, THE Repository SHALL record each removal batch as a separate git commit containing only that batch's deletions, such that each batch is individually revertable from git history.
7. WHEN the cleanup completes, THE Repository SHALL ensure that every path referenced by a retained skill or by the runtime resolves on disk, with zero unresolved references remaining. (Property 5)

### Requirement 11: Model and Harness Agnosticism

**User Story:** As a user running non-Anthropic models and varied editors, I want the discipline delivered without Anthropic-specific coupling, so that the same runtime works across models and harnesses.

#### Acceptance Criteria

1. THE TrueNorth_Server SHALL deliver skills and instructions containing no Anthropic-specific XML tags and no meta-instruction scaffolding directed at a specific model vendor.
2. WHEN a client request originates from any of the model families OpenAI o-series, GPT-4o, Gemini, DeepSeek, or locally hosted open-weight models, THE TrueNorth_Server SHALL return the requested skills and instructions with identical content regardless of the requesting model family.
3. WHEN a client request originates from any of the harnesses Kiro, Cursor, Claude Code, Zed, Neovim, or pi, THE TrueNorth_Server SHALL return the requested skills and instructions with identical content regardless of the requesting harness.
4. WHILE running gates or scanning governed target-project code for ontology drift, THE TrueNorth_Server SHALL apply language-agnostic processing that requires no per-language configuration and does not reject code based on its source programming language.
5. IF the requesting model family or harness cannot be identified from a client request, THEN THE TrueNorth_Server SHALL deliver the default language-agnostic skills and instructions and SHALL indicate in the response that no model-specific or harness-specific adaptation was applied.
