# Implementation Plan: Agent Workspace Profiles

## Overview

This plan implements the split between a human-facing `specs/` layer and a
machine-facing `.agent/` layer on top of the TrueNorth-MCP refactor runtime. It
adds the single write guard, relocates the cockpit into `.agent/`, adds five fixed
methodology profiles, replaces the mandatory `epic_id` with a neutral grouping key,
adds the greenfield scaffold tool, the two git hooks, the external-tracker bug
reference, and the read-only `truenorth://adr` resource. It then finishes the paused
skill rewrite and `specs/` cleanup and authors the repo-root `AGENTS.md`.

The runtime crate is written in **Rust** (rmcp, serde, serde_yaml, schemars, tokio,
notify, regex). These languages are fixed by the design (Part II module layout). No
pseudocode language selection is required.

Tasks build incrementally. The engine layer (write guard, config, profiles, cockpit
relocation, watcher) lands and tests before the tool and resource layers that consume
it. The tools and the ADR resource wire into the stdio server. Per the design's
**parity-before-removal** rule (§10 sequencing), the destructive `specs/` cleanup is
sequenced last, after the runtime re-point and the skill rewrite land, so nothing
breaks a live reference mid-cleanup. Each removal batch ships as its own revertible
commit.

Property-based and golden tests target the design correctness properties, which
continue the refactor's P1..P5: P6 (write guard), P7 (cockpit relocation
backward-compat), P8 (neutral grouping backward-compat), P9 (grouping validation),
P10 (profile resolution), P11 (post-merge sweep safety), P12 (commit-msg hook),
P13 (scaffold non-destructive), P14 (bug reference validation), P15 (no dangling
skill reference).

## Tasks

- [x] 1. Engine write guard: the single `.agent/` write path
  - [x] 1.1 Implement `engine::agent_ws` (`agent_ws.rs`)
    - Define `AGENT_DIR = ".agent"` and `TELEMETRY_AREA = "telemetry"` constants
    - Implement `write_under_agent(repo_root, rel_path, contents)` that normalizes the joined path, rejects `..` traversal and any absolute path that escapes `.agent/`, and delegates the byte write to the existing `engine::cockpit::write_atomic` helper
    - Return `WriteGuardError::OutsideAgent` naming the target when the path escapes; write nothing on reject so every target stays unchanged
    - Implement `is_excluded_read(rel_path)` returning true exactly for paths under `.agent/telemetry/`
    - Implement `read_layout(repo_root)` that parses `.agent/layout.yml`, confirms every area and file named in Requirement 1.5 through 1.9 is present, and on absence returns an error naming the absent path and the expected layout while retaining the last valid contract state in memory
    - `unwrap`/`expect`/`panic!` are banned; return typed `Result`
    - _Requirements: 1.1, 1.2, 1.3, 1.9, 1.10, 1.11, 1.12_

  - [x]\* 1.2 Write property test for the write guard
    - **Property 6: Runtime writes only under `.agent/`**
    - Generate relative and absolute paths, some under `.agent/`, some escaping via `..` or a `specs/` prefix; assert `write_under_agent` succeeds exactly for paths under `.agent/` and writes nothing on reject
    - Assert `is_excluded_read` is true exactly for `.agent/telemetry/` paths
    - **Validates: Requirements 1.2, 1.3, 1.4, 1.10**

  - [x]\* 1.3 Write unit tests for the layout contract read
    - Test the present-contract pass and the absent-entry error naming the missing path and the expected layout
    - Test that the last valid contract state is retained on a rejected read
    - _Requirements: 1.11, 1.12_

  - [x] 1.4 Add the audited repo-seed write path for the scaffold
    - Implement `engine::agent_ws::write_repo_seed(repo_root, rel_path, contents)` gated behind an explicit `allow_repo_root_seed` flag, used only by the scaffold to emit root docs, `.githooks/`, and `.github/` outside `.agent/`
    - Runtime tools never call `write_repo_seed`; assert this with a test in task 6
    - _Requirements: 5.5, 5.6, 5.8, 5.9_

- [x] 2. Engine config: markers and git scope re-point
  - [x] 2.1 Update `engine::config` markers and git scope
    - Set `MARKER_DIRS = [".agent", "specs", "skills"]` and `GIT_SCOPE_DIRS = [".agent", "specs", "skills"]`
    - Update `is_valid_repo_root` to require all three markers
    - _Requirements: 2.7, 2.8_

  - [x]\* 2.2 Write unit tests for config markers
    - Test that a root missing any of the three markers fails the valid-root check
    - Test that git scope includes `.agent/`, `specs/`, and `skills/`
    - _Requirements: 2.7, 2.8_

- [x] 3. Engine profiles: five built-ins and resolution
  - [x] 3.1 Implement `engine::profile` (`profile.rs`)
    - Define `GroupingVocab`, `GroupingRule`, and the `Profile` data struct per §3.1
    - Define the five `const` profiles and `ALL_PROFILES`: epic-based, issue-per-task, kanban, milestone-based, generic, each with vocab, rule, starter files, branch pattern, and `require_issue_id`
    - Implement `by_name(name)` returning `None` for an unknown name
    - Implement `resolve_active(repo_root)` reading `.agent/profile.yml`; absent config resolves to `ISSUE_PER_TASK`; an unknown name returns an error naming the value and the five valid names with no partial state
    - Reject any request to define a custom profile: there is no registration path
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

  - [x]\* 3.2 Write property test for profile resolution
    - **Property 10: Profile default and unknown-name resolution**
    - For any name outside the five, assert `by_name` is `None` and `resolve_active` errors naming the five valid names; for absent config assert issue-per-task; for each of the five names assert resolution returns that profile
    - **Validates: Requirements 3.2, 3.5, 3.6**

- [x] 4. Cockpit relocation into `.agent/`
  - [x] 4.1 Re-point cockpit backing paths
    - Re-point `ResourceDoc::backing_path` so `truenorth://state` resolves `.agent/tasks/state.yml`, `truenorth://cockpit` resolves `.agent/tasks/release-plan.yml`, and `truenorth://ontology` resolves `.agent/ontology.yml`
    - Re-point `engine::cockpit::state_path` and `release_plan_path` to the same `.agent/tasks/` paths
    - Update `display_backing` message strings to match
    - Route the ontology create-on-read through `write_under_agent`; a create failure returns a resource read error naming the ontology path and cause, leaves existing `.agent/` files unchanged, and retains the last good ontology content
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

  - [x] 4.2 Implement the legacy `specs/` cockpit read
    - When a backing file is absent under `.agent/` and a legacy file is present under `specs/`, `read_state` and `read_release_plan` fall back to the legacy path, parse it, and map its content onto the `.agent/` model through the `#[serde(flatten)]` catch-all
    - Preserve every unknown field and the `bigpowers_version` value with their original keys and values
    - A malformed legacy file returns a resource read error naming the file and the parse cause, leaves `.agent/` files unchanged, and retains the last good content
    - A subsequent runtime write goes to `.agent/` only; the legacy `specs/` file is never mutated
    - _Requirements: 2.9, 2.10, 2.11, 2.13, 1.4_

  - [x]\* 4.3 Write property test for cockpit relocation backward-compat
    - **Property 7: Cockpit relocation preserves unknown fields and version**
    - Generate legacy cockpit maps with random unknown fields and a `bigpowers_version`; read and map; assert every unknown field and the version value survive
    - Edge case: a malformed legacy YAML yields the naming error and a retained last-good, with `.agent/` unchanged
    - **Validates: Requirements 2.9, 2.11, 2.13, 2.10**

  - [x] 4.4 Re-point the watcher path mapping
    - Update `engine::watcher::map_path_to_uri` so `.agent/tasks/state.yml`, `.agent/ontology.yml`, `.agent/tasks/release-plan.yml`, and `.agent/product/` map to their resource URIs, and the watcher no longer watches `specs/product/`
    - Keep `CONVENTIONS.md` mapping unchanged
    - _Requirements: 2.6, 2.12_

  - [x]\* 4.5 Write unit tests for the watcher re-point
    - Test that each relocated `.agent/` path maps to the correct URI and that `specs/product/` is no longer watched
    - _Requirements: 2.6, 2.12_

- [x] 5. Neutral grouping key in `record_task`
  - [x] 5.1 Replace `epic_id` with the neutral grouping key in `tools/lifecycle.rs`
    - Replace the `epic_id` field of `RecordTaskArgs` with `group_id` (optional, 1 to 200 chars), `group_kind` (optional enum epic|sprint|milestone|ticket), and a retained legacy `epic_id` field, keeping the strict `schemars` schema
    - Validate before any mutation: map legacy `epic_id` to `group_kind = epic` and `group_id = epic_id` when `epic_id` is present and the neutral fields are absent
    - Resolve the active profile; when grouping is `Required` and `group_id` is absent, reject naming the missing grouping key and preserve the pre-call state; when `Optional`, accept the omission
    - Reject an empty or over-200-char `group_id` naming the value and its bound; reject a `group_kind` outside the allowed set naming the value and the set; reject a `group_kind` absent from the active profile vocabulary naming the mismatch and the vocabulary; preserve the pre-call state on every reject
    - Update `engine::cockpit::apply_task` to write `group_id` and `group_kind` in place of `epic_id`, preserving every other field
    - On read of a cockpit file carrying a legacy `active_epic`, map it onto the neutral grouping model and preserve the original `active_epic` value through the catch-all
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9_

  - [x]\* 5.2 Write property test for neutral grouping backward-compat
    - **Property 8: Neutral grouping preserves legacy mapping and fields**
    - For any legacy `epic_id`, assert the recorded task carries `group_kind = epic` and `group_id = epic_id`; for any cockpit with `active_epic`, assert the value is preserved; for any write, assert every pre-state unknown field survives
    - **Validates: Requirements 4.7, 4.8, 4.9**

  - [x]\* 5.3 Write property test for grouping-key validation
    - **Property 9: Grouping-key validation rejects invalid input and preserves state**
    - Generate valid and invalid grouping inputs across the five profiles; assert accept/reject matches the rule and the cockpit file is unchanged on reject
    - **Validates: Requirements 4.2, 4.3, 4.4, 4.5, 4.6**

- [x] 6. Checkpoint - Make sure that engine, profile, and grouping tests pass
  - Make sure that all tests pass. Ask the user if questions arise.

- [x] 7. External-tracker bug reference tool
  - [x] 7.1 Implement `tools/bugref.rs`
    - Define `RecordBugArgs` (`id`, `external_link`, `status`, `linked_ref`, optional `tags`) and the `BugStatus` enum per §9, with a strict `schemars` schema
    - Validate before any write: id length 1 to 200; `external_link` parses as an absolute URL; `status` is one of the enum; `linked_ref` resolves to an existing task or group id in `.agent/tasks/`
    - Store the accepted record into `.agent/tasks/bugs.yml` through `write_under_agent`, storing caller-supplied tags as given
    - Make no network request, integrate no tracker API, and store no tracker credential
    - On a missing or invalid field, return an error naming the offending field and its expected shape, and leave `.agent/tasks/bugs.yml` in its pre-invocation state
    - Register the tool in `src/tools/mod.rs`
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.6, 8.7_

  - [x]\* 7.2 Write property test for bug-reference validation
    - **Property 14: Bug-reference validation and tag preservation**
    - Generate valid and invalid bug references; assert accept/reject matches the four field rules, tags round-trip on acceptance, and `bugs.yml` is unchanged on reject
    - **Validates: Requirements 8.3, 8.6, 8.7**

- [x] 8. ADR read-only resource
  - [x] 8.1 Implement `resources/adr.rs` and register the resource
    - Add an `Adr` variant to `ResourceDoc`, grow `ALL_RESOURCES` to five, and resolve `backing_path` to `repo_root/specs/adr`
    - Read and concatenate the ADR files in `specs/adr/` on `resources/read`
    - Absent `specs/adr/`: return a resource read error naming the missing directory and keep serving other resources
    - ADR file parse failure: return a resource read error naming the file and the failure, retain the last successfully parsed content in the cache, and keep serving other resources
    - The resource is read-only: a write through it returns an error identifying the read-only resource, and every file under `specs/adr/` stays unchanged; the write guard also rejects any `specs/` target
    - Add `specs/adr/` mapping to `map_path_to_uri` so the watcher emits `notifications/resources/updated` for `truenorth://adr` within 1 second of a change
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6_

  - [x]\* 8.2 Write integration and property coverage for the ADR resource
    - **Property 6: Runtime writes only under `.agent/`** — assert a write through `truenorth://adr` is rejected and `specs/adr/` bytes stay unchanged
    - Edit a file under `specs/adr/` and assert `resources/updated` fires within 1 second against a temp repo
    - Assert the absent-directory and parse-failure read errors, the retained last-good, and that other resources keep serving
    - **Validates: Requirements 9.3, 9.4, 9.5, 9.6**

- [x] 9. Greenfield scaffold tool
  - [x] 9.1 Implement `tools/scaffold.rs` core emission
    - Define `ScaffoldArgs` (optional `profile`, 1 to 64 chars) with a strict `schemars` schema
    - Resolve the profile; absent uses issue-per-task; an unknown name makes no file change and returns an error naming the value and the known names
    - Emit the `.agent/` tree and the cockpit seed files for the profile's `starter_files`; produce a language-agnostic scaffold with no `Cargo.toml`, no `package.json`, and no source tree
    - Emit root `AGENTS.md` and `CONVENTIONS.md` wired to `.agent/`, templated per profile, never wired to `specs/`, through `write_repo_seed`
    - Non-destructive: leave any existing target path or existing `.agent/` tree unchanged and print a skip message naming the skipped path
    - Register the tool in `src/tools/mod.rs`
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.12_

  - [x] 9.2 Emit git hooks and `.github/` templates from the scaffold
    - Emit `.githooks/commit-msg` and `.githooks/post-merge`, templated per profile, through `write_repo_seed`
    - Print `git config core.hooksPath .githooks` to standard output and do not run it
    - Emit `.github/commit-template.md` and `.github/pull-request-template.md` as neutral, profile-identical templates stating the atomic-and-conventional-commit rule
    - Emit `.github/ISSUE_TEMPLATE/` fresh: a bug form aligned to the bug reference, a feature form, and a `config.yml` with placeholder contact links; generic structure only, no copied domain content, no hardcoded URLs
    - Include a grouping field in the issue forms when the profile vocabulary is epic or milestone; omit the issue-id field when the profile is kanban or generic
    - _Requirements: 5.6, 5.7, 5.8, 5.9, 5.10, 5.11_

  - [x]\* 9.3 Write property and golden tests for the scaffold
    - **Property 13: Greenfield scaffold is non-destructive** — for any pre-existing subset of scaffold targets, assert the bytes stay unchanged and a skip message names each; assert an unknown profile makes no file change
    - Golden: snapshot the emitted `.agent/` tree, root docs, the two hooks, and the `.github/` templates per profile; assert no `Cargo.toml`, `package.json`, or source tree, and that `git config core.hooksPath .githooks` is printed, not run
    - Assert that runtime tools never call `write_repo_seed`
    - **Validates: Requirements 5.2, 5.4, 5.7, 5.12**

- [x] 10. Emitted git hooks: content and behavior
  - [x] 10.1 Author the `commit-msg` hook template
    - Read the message file passed as `$1`; accept generated merge and revert messages with exit 0
    - Accept a subject that uses a type in {feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert} and matches `type(scope): description`; reject otherwise with a non-zero exit and the stated error
    - Template `REQUIRE_ISSUE_ID` to `yes` for epic-based, issue-per-task, milestone-based and `no` for kanban, generic; when `yes`, require an issue or ticket id reference and reject its absence with a non-zero exit; when `no`, accept a message with no id and exit 0
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9, 6.10_

  - [x] 10.2 Author the `post-merge` hook template
    - Sweep only while on the trunk; exit 0 and delete nothing on a detached HEAD or an undeterminable current branch
    - Match candidate branches against the profile branch pattern; never delete the current branch or the trunk
    - Delete a local topic branch only when it is provably present on the trunk, where provably present means the branch tip is an ancestor of the trunk tip or `git diff trunk..branch` reports no differences; retain any branch not provably present
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8_

  - [x]\* 10.3 Write property test for the commit-msg hook decision
    - **Property 12: Commit-msg hook decision**
    - Generate subjects (valid/invalid type, with/without scope, with/without id) across profiles and run the emitted hook against temp repos; assert the exit code matches the rule and merge/revert subjects always exit 0
    - **Validates: Requirements 6.3, 6.4, 6.5, 6.7, 6.8, 6.9, 6.10**

  - [x]\* 10.4 Write property test for the post-merge sweep safety
    - **Property 11: Post-merge sweep safety**
    - Build temp repos with random branch and merge topologies and run the emitted hook; assert the current and trunk branches always survive, only ancestor-or-empty-diff matching branches are deleted, and a detached HEAD deletes nothing
    - **Validates: Requirements 7.2, 7.3, 7.4, 7.5, 7.6, 7.8**

- [x] 11. Wire the new tools and resource into the server
  - [x] 11.1 Aggregate the scaffold, bug-reference tool, and ADR resource in `src/index.rs`
    - Register `truenorth_scaffold_project` and `truenorth_record_bug` in the tool router, add the `Adr` resource to the `ServerHandler`, and confirm the watcher watches `specs/adr/` and the relocated `.agent/` paths
    - _Requirements: 5.1, 8.1, 9.2, 2.6_

  - [x]\* 11.2 Write an MCP round-trip integration test
    - Drive `resources/read` for each relocated resource against a temp repo seeded under `.agent/`, and against a legacy-`specs/` temp repo, asserting the backward-compat read
    - Drive `truenorth_record_bug` and `truenorth_scaffold_project` end to end against a temp repo
    - _Requirements: 2.9, 8.1, 5.3_

- [x] 12. Version control the `.agent/` directory
  - [x] 12.1 Remove the `.agent` ignore entries
    - Remove the `.gitignore` entries that ignore `.agent` and `.agents` so the `.agent/` tree tracks under version control
    - Assert `git status` reports the `.agent/` tree as trackable
    - _Requirements: 1.13_

- [x] 13. Checkpoint - Confirm the runtime re-point holds before cleanup
  - Make sure that all tests pass and every retained skill and runtime reference still resolves. Ask the user if questions arise. Do not start the destructive `specs/` cleanup (tasks 15 to 20) unless the runtime re-point and the skill rewrite hold.

- [x] 14. Skill rewrite off removed `specs/` paths
  - [x] 14.1 Neutralize retained skills of mandatory epics and removed paths
    - Reword every retained `skills/*/SKILL.md` so it organizes around the neutral grouping model with zero references to mandatory epics
    - Reword each example path that names a removed directory to a neutral placeholder or the `.agent/` path, so no example path resolves to a removed directory (for example the `specs/bugs/`, `specs/epics/`, and `specs/release-plan.yaml` references in `seed-conventions`, `slice-tasks`, and `plan-release`)
    - Commit as its own revertible batch
    - _Requirements: 10.1, 10.4, 10.10_

- [x] 15. Migrate cockpit state and the product concept into `.agent/`
  - [x] 15.1 Move the cockpit and product files under `.agent/`
    - Move `state.yaml`, `release-plan.yaml`, `ontology.yaml`, and `execution-status.yaml` into `.agent/tasks/` and `.agent/ontology.yml`; migrate the product concept into `.agent/product/` and remove the bigpowers `specs/product/` content
    - On a failed migration, leave the tree in its pre-batch state and return an error naming the failed batch
    - Commit as its own revertible batch
    - _Requirements: 2.1, 2.2, 2.3, 2.12, 10.9, 10.12_

- [ ] 16. Remove `specs/bugs/` and the `.okf` sidecars
  - [ ] 16.1 Remove the bug narrative directory and sidecar format
    - Remove `specs/bugs/` and every `*.okf.md` sidecar; do not carry the `.okf` sidecar format into TrueNorth
    - Commit as its own revertible batch
    - _Requirements: 8.5, 10.10_

- [ ] 17. Remove generated wikis and JSON side-cars
  - [ ] 17.1 Remove the regeneratable artifacts
    - Remove `verifications/`, `epics/`, `codebase-wiki/`, and the generated `adr-wiki/`, `epics-wiki/`, `skills-wiki/`, and `conventions-wiki/` directories
    - Remove the JSON side-cars: `blind-spots`, `drift-report`, `skill-graph`, `receipts`, `rule-matrix`, `import-boundaries`, and `traceability-matrix`
    - Commit as its own revertible batch
    - _Requirements: 10.5, 10.6, 10.10_

- [ ] 18. Remove process docs and stray artifacts
  - [ ] 18.1 Remove the process exhaust
    - Remove the `*_LATEST.md` files, the process docs, the stray yaml files `agent-locks`, `planning-status`, and `tombstones`, and the `viz.html` file
    - Commit as its own revertible batch
    - _Requirements: 10.7, 10.10_

- [ ] 19. Remove upstream infra directories and `metrics/`
  - [ ] 19.1 Remove the upstream infra directories
    - Remove `benchmarks/`, `archive/`, `migrations/`, `security/`, `tech-architecture/`, `wayfinder/`, `workflows/`, `agent-guide/`, `templates/`, and `metrics/`
    - Commit as its own revertible batch
    - _Requirements: 10.8, 12.2, 10.10_

- [ ] 20. Migrate and author ADRs
  - [ ] 20.1 Migrate, supersede, and add ADRs under `specs/adr/`
    - Migrate the still-valid bigpowers ADRs into `specs/adr/`: 0001 verb-noun-naming, 0003 prescriptive-core-loop, 0004 context-isolation, 0005 hard-gate-mandate, 0006 model-routing
    - Mark ADR 0002 local-first-specs and ADR 0007 agents-md-spine superseded, each with a pointer to the replacing decision
    - Add new ADRs for this spec: the `.agent`-versus-`specs` split, methodology profiles, tracker-owns-bugs, and cockpit relocation
    - Commit as its own revertible batch
    - _Requirements: 9.7, 9.8, 9.9, 10.10_

- [ ] 21. Author the repo-root AGENTS.md
  - [ ] 21.1 Write the repo-root `AGENTS.md` against the final layout
    - Author a repo-root `AGENTS.md` for the TrueNorth-MCP project, wired to the final `.agent/` layout, never wired to `specs/`
    - Commit as its own revertible batch
    - _Requirements: 11.1, 11.2_

- [ ] 22. Final verification - No dangling skill reference
  - [ ] 22.1 Assert every retained skill reference resolves
    - **Property 15: No dangling skill reference after cleanup**
    - Grep every retained `skills/*/SKILL.md` for referenced paths, assert each resolves on disk, and assert the unresolved count is zero, counting the approximately 20 references that dangle today
    - _Requirements: 10.2, 10.3, 10.4_

  - [ ] 22.2 Final checkpoint - Make sure that all tests pass
    - Make sure that all tests pass and no dangling references remain. Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional (test tasks) and can be skipped for a faster MVP. Core implementation and cleanup tasks are never optional.
- The cleanup batches (tasks 14 to 21) are higher-risk and destructive. They are sequenced after the runtime re-point (tasks 1 to 12) and gated on the checkpoint in task 13, so nothing breaks a live reference mid-cleanup (parity-before-removal).
- Each cleanup removal batch is its own git commit so it is individually revertible (Requirement 10.10, 10.11). Task 22.1 asserts the zero-dangling end-state.
- Each task references specific requirement clauses for traceability.
- Property tests validate the design correctness properties: P6 (1.2), P7 (4.3), P8 (5.2), P9 (5.3), P10 (3.2), P11 (10.4), P12 (10.3), P13 (9.3), P14 (7.2), P15 (22.1).

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "2.1", "3.1"] },
    { "id": 1, "tasks": ["1.2", "1.3", "1.4", "2.2", "3.2", "4.1", "4.4"] },
    { "id": 2, "tasks": ["4.2", "4.5", "5.1", "7.1", "8.1", "10.1", "10.2"] },
    { "id": 3, "tasks": ["4.3", "5.2", "5.3", "7.2", "8.2", "9.1", "10.3", "10.4"] },
    { "id": 4, "tasks": ["9.2", "11.1", "12.1"] },
    { "id": 5, "tasks": ["9.3", "11.2"] },
    { "id": 6, "tasks": ["14.1"] },
    { "id": 7, "tasks": ["15.1"] },
    { "id": 8, "tasks": ["16.1"] },
    { "id": 9, "tasks": ["17.1"] },
    { "id": 10, "tasks": ["18.1"] },
    { "id": 11, "tasks": ["19.1"] },
    { "id": 12, "tasks": ["20.1"] },
    { "id": 13, "tasks": ["21.1"] },
    { "id": 14, "tasks": ["22.1"] }
  ]
}
```
