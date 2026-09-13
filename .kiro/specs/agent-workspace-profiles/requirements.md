# Requirements Document

## Introduction

Agent Workspace Profiles extends the TrueNorth-MCP runtime with a clean split between a human-facing layer and a machine-facing layer. The top-level `specs/` directory becomes human-authored narrative. A new `.agent/` directory holds the controlled subset that agents read, watch, and write. The runtime writes only under `.agent/`. The runtime can read human-authored files under `specs/`, but never mutates anything under `specs/`.

This feature builds on the TrueNorth-MCP refactor (`.kiro/specs/truenorth-mcp-refactor/`). It relocates the cockpit into `.agent/`, adds five fixed methodology profiles, de-couples the tool contract from mandatory epics through a neutral grouping key, adds a greenfield scaffolding skill, defines two git hooks, adopts external-tracker bug tracking, migrates the ADR practice into a read-only resource, and finishes the paused skill rewrite and `specs/` cleanup. All backward-compatibility guarantees from the refactor spec carry forward.

These requirements cover eight areas: the `.agent/` layout contract, cockpit relocation, methodology profiles and neutral grouping, the greenfield scaffolding skill, the two git hooks, external-tracker bug references, the ADR resource, and the skill rewrite with `specs/` cleanup. Design correctness properties are noted per clause so the design phase can attach P-numbers. This spec follows the strict-tier house writing rules.

## Glossary

- **agent workspace**: The `.agent/` directory (singular "agent") at the repository root. This directory holds the controlled subset of files that the runtime reads, watches, and writes. The runtime writes only under this directory.
- **human-facing specs**: The top-level `specs/` directory holding human-authored narrative, for example ADRs. The runtime can read these files, but never mutates them.
- **methodology profile**: A named data record that declares a project workflow shape. A profile carries a methodology name, a grouping vocabulary, whether grouping is required or optional, and a starter file set for `.agent/`.
- **grouping key**: The neutral field that associates a task with an optional group. The grouping key replaces the mandatory `epic_id`. It carries a `group_id` and an optional `group_kind`.
- **grouping vocabulary**: The label a profile uses for a group, one of epic, sprint, milestone, ticket, or none.
- **cockpit**: The set of load-bearing state files the runtime reads and writes. Under this spec, the cockpit files move from `specs/` into `.agent/`.
- **bug reference**: A lean record of one bug stored in the repository. A bug reference carries an id, an external link, a status, the linked task or group, and optional caller-supplied tags. The external tracker is the source of truth.
- **ADR resource**: A read-only MCP resource that resolves the `specs/adr/` path and serves human-authored ADRs to agents.
- **greenfield scaffold**: The action of a new skill that seeds a brand-new project with TrueNorth workflow conventions before any code is written.
- **commit-msg hook**: The git hook installed at the `commit-msg` stage. This hook reads the commit-message file passed as `$1` and validates Conventional Commits format and a profile-driven id reference.
- **post-merge sweep**: The action of the git `post-merge` hook that deletes local topic branches whose content is provably already on the trunk branch.
- **trunk branch**: The always-releasable primary branch (for example `main`) in trunk-based development.
- **profile-default resolution**: The rule the runtime applies when no methodology is declared. The runtime resolves to the issue-per-task profile with optional grouping.

## Requirements

### Requirement 1: The `.agent/` Layout Contract

**User Story:** As a maintainer, I want a language-agnostic `.agent/` layout contract, so that both the Rust runtime and any tooling read and write the same controlled workspace structure.

#### Acceptance Criteria

1. THE Runtime SHALL define the `.agent/` directory at the repository root as the single agent workspace that the runtime reads, watches, and writes.
2. THE Runtime SHALL write only under `.agent/`, and SHALL NOT write any path outside `.agent/`. (Property: runtime-writes-only-under-agent)
3. IF the runtime attempts a write to a path outside `.agent/`, THEN THE Runtime SHALL reject the write, leave every target path unchanged, and return an error that names the rejected path and states that writes must stay under `.agent/`. (Property: runtime-writes-only-under-agent)
4. WHERE a human-authored file under `specs/` is read, THE Runtime SHALL read that file without mutating any path under `specs/`. (Property: runtime-writes-only-under-agent)
5. THE Agent_Workspace SHALL define a `config/` area holding `rules.yml` for token caps, human-approval gates, and protected paths, plus `context-map.yaml`, `context-gates.yml`, and `lint-standards.md`.
6. THE Agent_Workspace SHALL define a `spec/` area holding `requirements.md`, `architecture.md`, `definition-of-ready.md`, and `definition-of-done.md`.
7. THE Agent_Workspace SHALL define a `tasks/` area holding `state.yml` with a TDD loop, `active-feature.yaml`, and `backlog.yml`.
8. THE Agent_Workspace SHALL define a `memories/` area holding `lessons.md` and `glossary.md`.
9. THE Agent_Workspace SHALL define a `telemetry/` area holding `runs.yml` for the agent cost audit, and SHALL exclude the `telemetry/` area from agent reads.
10. IF an agent read targets any path under the `telemetry/` area, THEN THE Runtime SHALL reject the read, return no `telemetry/` content, and return an error that names the rejected path and states that the `telemetry/` area is excluded from agent reads.
11. THE Agent_Workspace SHALL define the layout contract as language-agnostic data that a reader in any language can parse, and the runtime reading the contract is Rust.
12. IF a required area or file named in criteria 5 through 9 is absent when the runtime reads the layout contract, THEN THE Runtime SHALL reject the contract, retain the last valid contract state, and return an error that names the absent path and states the expected layout.
13. THE Repository SHALL track the `.agent/` directory under version control, and SHALL remove any `.gitignore` entry that ignores `.agent` or `.agents`.

### Requirement 2: Relocate Cockpit Files into `.agent/`

**User Story:** As an operator, I want the cockpit files moved from `specs/` into `.agent/`, so that the machine-facing state lives in the controlled agent workspace while `specs/` stays human-authored.

#### Acceptance Criteria

1. THE Runtime SHALL resolve the `truenorth://state` backing file under `.agent/` instead of `specs/state.yaml`.
2. THE Runtime SHALL resolve the `truenorth://cockpit` backing file under `.agent/` instead of `specs/release-plan.yaml`.
3. THE Runtime SHALL resolve the `truenorth://ontology` backing file under `.agent/` instead of `specs/ontology.yaml`.
4. WHEN a `resources/read` targets `truenorth://ontology` and the backing file is absent, THE Runtime SHALL create that file under `.agent/`.
5. IF the Runtime cannot create the ontology file under `.agent/`, THEN THE Runtime SHALL return a resource read error that names the ontology path and the failure cause, SHALL leave existing `.agent/` files unchanged, and SHALL retain the last good ontology content (refactor spec Requirement 5.7).
6. THE Runtime SHALL map every watched cockpit path under `.agent/` to its resource URI, including the relocated product path.
7. THE Runtime SHALL include `.agent/` in the repository-root marker directories.
8. THE Runtime SHALL set the git-scope directories to `.agent/`, `specs/`, and `skills/`, because the runtime reads ADR content under `specs/`.
9. IF an existing bigpowers cockpit is present under `specs/`, THEN THE Runtime SHALL read that cockpit without error and map its content onto the `.agent/` layout. (Property: cockpit-relocation-backward-compat)
10. IF a legacy cockpit read under `specs/` is malformed, THEN THE Runtime SHALL return a resource read error that names the offending file and the parse failure cause, SHALL leave existing `.agent/` files unchanged, and SHALL retain the last good content (refactor spec Requirement 2.12).
11. WHEN the runtime maps a legacy cockpit into `.agent/`, THE Runtime SHALL preserve every unknown field with its original key and value, consistent with refactor spec Requirement 9. (Property: cockpit-relocation-backward-compat)
12. THE Runtime SHALL move the product concept into `.agent/`, such that the runtime watches the product path under `.agent/` and no longer watches `specs/product/`.
13. WHILE reading a relocated cockpit file, THE Runtime SHALL preserve a `bigpowers_version` key with its original value, consistent with refactor spec Requirement 9.

### Requirement 3: Methodology Profiles

**User Story:** As a project owner, I want five fixed methodology profiles as data, so that the runtime and the scaffolder share one declaration of the project workflow shape without a code-plugin surface.

#### Acceptance Criteria

1. THE Runtime SHALL define exactly five built-in methodology profiles: epic-based, issue-per-task, kanban continuous-flow, milestone/release-based, and generic no-grouping.
2. THE Runtime SHALL treat issue-per-task as the default methodology profile.
3. THE Runtime SHALL represent each methodology profile as data carrying a methodology name, a grouping vocabulary of exactly one of epic, sprint, milestone, ticket, or none, a flag stating whether grouping is required or optional, and a starter file set for `.agent/`.
4. WHEN the Runtime starts, THE Runtime SHALL read the declared methodology profile name from `.agent/` config.
5. IF no methodology profile is declared in `.agent/` config, THEN THE Runtime SHALL resolve to the issue-per-task profile with optional grouping. (Property: profile-default-resolution)
6. IF the declared methodology profile name is not one of the five built-in profiles, THEN THE Runtime SHALL reject the config, retain no partial profile state, and return an error indicating the unknown profile name and the five valid profile names. (Property: profile-default-resolution)
7. THE Runtime SHALL reject any request to define a user-supplied custom methodology profile, and SHALL treat custom profiles as a future extension outside this spec.

### Requirement 4: Neutral Grouping Key

**User Story:** As an AI agent, I want a neutral grouping key instead of a mandatory epic id, so that I record tasks under any methodology profile while legacy epic calls keep working.

#### Acceptance Criteria

1. THE Runtime SHALL replace the mandatory `epic_id` field of `truenorth_record_task` with a neutral grouping key that carries a `group_id` string of 1 to 200 characters and an optional `group_kind` from the set epic, sprint, milestone, or ticket.
2. WHERE the active methodology profile marks grouping as optional, THE Runtime SHALL accept a `truenorth_record_task` call that omits the grouping key.
3. WHERE the active methodology profile marks grouping as required, THE Runtime SHALL require the grouping key on a `truenorth_record_task` call, and IF the grouping key is absent, THEN THE Runtime SHALL reject the call with an error that identifies the missing grouping key and preserve the pre-call state.
4. IF a `truenorth_record_task` call supplies a `group_id` that is empty or longer than 200 characters, THEN THE Runtime SHALL reject the call with an error that identifies the invalid `group_id` and its bound, and preserve the pre-call state.
5. IF a `truenorth_record_task` call supplies a `group_kind` outside the set epic, sprint, milestone, or ticket, THEN THE Runtime SHALL reject the call with an error that identifies the invalid `group_kind` and the allowed set, and preserve the pre-call state.
6. IF a `truenorth_record_task` call supplies a `group_kind` that is absent from the active methodology profile grouping vocabulary, THEN THE Runtime SHALL reject the call with an error that identifies the mismatched `group_kind` and the profile vocabulary, and preserve the pre-call state.
7. WHEN a `truenorth_record_task` call supplies a legacy `epic_id`, THE Runtime SHALL accept the call and map the `epic_id` onto the neutral grouping key with `group_kind` set to epic. (Property: neutral-grouping-backward-compat)
8. WHEN the runtime reads a cockpit file carrying a legacy `active_epic` field, THE Runtime SHALL map that field onto the neutral grouping model and preserve the original `active_epic` value. (Property: neutral-grouping-backward-compat)
9. WHEN the runtime writes the neutral grouping key, THE Runtime SHALL preserve every unknown field present in the pre-state, consistent with refactor spec Requirement 9. (Property: neutral-grouping-backward-compat)

### Requirement 5: Greenfield Scaffolding Skill

**User Story:** As a developer starting a new project, I want a greenfield scaffolding skill, so that a brand-new repository is seeded with TrueNorth workflow conventions before any code is written.

#### Acceptance Criteria

1. THE Greenfield_Scaffold SHALL accept a methodology profile name of 1 to 64 characters, and WHEN no profile name is supplied, THE Greenfield_Scaffold SHALL use the issue-per-task profile.
2. IF the supplied methodology profile name is not a known profile, THEN THE Greenfield_Scaffold SHALL make no file changes, and SHALL return an error that names the offending value and lists the known profile names.
3. THE Greenfield_Scaffold SHALL create the `.agent/` tree and the cockpit files seeded for the supplied methodology profile.
4. THE Greenfield_Scaffold SHALL produce a language-agnostic scaffold, and SHALL NOT create any code or project skeleton, including `Cargo.toml`, `package.json`, or a source tree.
5. THE Greenfield_Scaffold SHALL emit a root `AGENTS.md` and a root `CONVENTIONS.md` wired to `.agent/` and templated per methodology profile, and SHALL NOT wire either file to `specs/`.
6. THE Greenfield_Scaffold SHALL emit two git hooks into `.githooks/`, templated per methodology profile.
7. THE Greenfield_Scaffold SHALL print the `git config core.hooksPath .githooks` command to standard output, and SHALL NOT run that command.
8. THE Greenfield_Scaffold SHALL emit a `.github/commit-template.md` and a `.github/pull-request-template.md` as neutral workflow templates that enforce atomic and conventional commits, identical for every methodology profile.
9. THE Greenfield_Scaffold SHALL emit `.github/ISSUE_TEMPLATE/` as generic, profile-aware issue forms authored fresh, including a bug form aligned to the external-tracker bug reference, a feature form, and a `config.yml` with placeholder contact links.
10. THE Greenfield_Scaffold SHALL author every issue template with a generic structure only, and SHALL NOT copy any specific project's domain content or hardcoded URLs.
11. WHERE the active methodology profile uses a grouping vocabulary of epic or milestone, THE Greenfield_Scaffold SHALL include a grouping field in the issue forms, and WHERE the profile is kanban or no-grouping, THE Greenfield_Scaffold SHALL omit the issue-id field the matching hook does not enforce.
12. IF a target file or the `.agent/` tree already exists, THEN THE Greenfield_Scaffold SHALL leave that existing path unchanged, and SHALL print a skip message that names the skipped path.

### Requirement 6: Commit-Message Hook

**User Story:** As a maintainer, I want a commit-msg hook that validates commit format and a profile-driven id reference, so that every commit on the trunk obeys atomic and conventional commit rules.

#### Acceptance Criteria

1. THE Commit_Msg_Hook SHALL install at the `commit-msg` git stage.
2. WHEN git invokes the hook, THE Commit_Msg_Hook SHALL read the commit-message file passed as the first argument `$1`.
3. WHEN the commit message uses a type that is one of `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, or `revert`, and matches the Conventional Commits format `type(scope): description`, THE Commit_Msg_Hook SHALL treat the type and format as valid.
4. IF the commit message uses a type that is not one of `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, or `revert`, THEN THE Commit_Msg_Hook SHALL reject the commit with a non-zero exit status and an error that states the allowed type set.
5. IF the commit message does not match the Conventional Commits format `type(scope): description`, THEN THE Commit_Msg_Hook SHALL reject the commit with a non-zero exit status and an error that states the expected format.
6. WHERE the active methodology profile is epic-based, issue-per-task, or milestone/release-based, THE Commit_Msg_Hook SHALL require an issue or ticket id reference in the commit message.
7. WHERE the active methodology profile is epic-based, issue-per-task, or milestone/release-based, IF the issue or ticket id reference is absent, THEN THE Commit_Msg_Hook SHALL reject the commit with a non-zero exit status and an error that states the required reference.
8. WHERE the active methodology profile is kanban continuous-flow or generic no-grouping, THE Commit_Msg_Hook SHALL accept a commit message that carries no issue or ticket id reference, and SHALL return exit status 0.
9. WHEN the commit message matches the required format and reference rules for the active methodology profile, THE Commit_Msg_Hook SHALL accept the commit with exit status 0.
10. WHEN git supplies a generated merge-commit or revert-commit message, THE Commit_Msg_Hook SHALL accept the commit with exit status 0.

### Requirement 7: Post-Merge Branch Sweep

**User Story:** As a maintainer, I want a post-merge hook that sweeps merged topic branches, so that the trunk stays clean without risk of losing unmerged work.

#### Acceptance Criteria

1. THE Post_Merge_Hook SHALL install at the `post-merge` git stage.
2. WHILE the current branch is the trunk branch, THE Post_Merge_Hook SHALL sweep local topic branches. (Property: post-merge-sweep-safety)
3. IF the current branch cannot be determined, THEN THE Post_Merge_Hook SHALL exit with code 0 and delete no branch. (Property: post-merge-sweep-safety)
4. THE Post_Merge_Hook SHALL treat a local topic branch as provably present on the trunk branch when the branch tip is an ancestor of the trunk tip, OR when `git diff trunk..branch` reports no differences. (Property: post-merge-sweep-safety)
5. THE Post_Merge_Hook SHALL delete a local topic branch only when that branch is provably present on the trunk branch. (Property: post-merge-sweep-safety)
6. THE Post_Merge_Hook SHALL NOT delete the current branch, and SHALL NOT delete the trunk branch. (Property: post-merge-sweep-safety)
7. THE Post_Merge_Hook SHALL match swept branch names against the pattern declared by the active methodology profile.
8. IF a local topic branch is not provably present on the trunk branch, THEN THE Post_Merge_Hook SHALL retain that branch. (Property: post-merge-sweep-safety)

### Requirement 8: External-Tracker Bug References

**User Story:** As a maintainer, I want a lean in-repo bug reference backed by an external tracker, so that the repository records the link without carrying bug narratives or tracker credentials.

#### Acceptance Criteria

1. WHEN a caller creates a bug reference, THE Runtime SHALL store the bug reference as a cockpit file under `.agent/`, carrying an id, an external link, a status, the linked task or group, and optional caller-supplied tags.
2. THE Runtime SHALL treat the external tracker as the source of truth for bug detail, and SHALL NOT store an in-repo directory of bug narratives.
3. WHEN a caller supplies tags on a bug reference, THE Runtime SHALL store those tags as given.
4. THE Runtime SHALL NOT integrate a tracker API, make a network request, or store tracker credentials in this spec, and SHALL treat a live tracker sync as a future feature outside this spec.
5. THE Repository SHALL remove the bigpowers `specs/bugs/` directory and every `.okf.md` sidecar file, and SHALL NOT carry the `.okf` sidecar format into TrueNorth.
6. WHEN a caller creates a bug reference, THE Runtime SHALL validate that the id is a non-empty string of 1 to 200 characters, the external link is an absolute URL, the status is one of a bounded enumeration, and the linked task or group references an existing id.
7. IF a required field is missing or invalid, THEN THE Runtime SHALL reject the bug reference, return an error identifying the offending field and its expected shape, and leave the cockpit file in its pre-invocation state.

### Requirement 9: ADR Resource

**User Story:** As an AI agent, I want ADRs served as a read-only resource, so that I read the recorded architecture decisions without the ability to mutate the human-authored records.

#### Acceptance Criteria

1. THE Runtime SHALL keep the ADR practice, and SHALL store ADRs at `specs/adr/` as human-authored, git-tracked files.
2. THE Runtime SHALL serve ADRs through a read-only resource at the URI `truenorth://adr` that resolves the `specs/adr/` path, using the same pattern as `truenorth://conventions` resolving the root `CONVENTIONS.md`.
3. WHEN a file under `specs/adr/` changes on disk, THE Runtime SHALL emit a `notifications/resources/updated` notification for the `truenorth://adr` resource within 1 second of detecting the change.
4. IF a write is attempted through the `truenorth://adr` resource, THEN THE Runtime SHALL reject the write, leave every file under `specs/adr/` in its pre-attempt state, and return an error identifying the read-only resource. (Property: runtime-writes-only-under-agent)
5. IF the `specs/adr/` directory is absent when the `truenorth://adr` resource is read, THEN THE Runtime SHALL return a resource read error identifying the missing directory, and SHALL continue serving other resources without terminating.
6. IF an ADR file fails to parse when the `truenorth://adr` resource is read, THEN THE Runtime SHALL return a resource read error identifying the file and the failure, retain the last successfully parsed content in memory, and continue serving other resources without terminating.
7. THE Repository SHALL migrate the still-valid bigpowers ADRs 0001 verb-noun-naming, 0003 prescriptive-core-loop, 0004 context-isolation, 0005 hard-gate-mandate, and 0006 model-routing into `specs/adr/`.
8. THE Repository SHALL mark ADR 0002 local-first-specs and ADR 0007 agents-md-spine as superseded, and SHALL add a pointer from each superseded ADR to the new decision that replaces it.
9. THE Repository SHALL add new ADRs recording the key decisions of this spec: the `.agent`-versus-`specs` split, methodology profiles, tracker-owns-bugs, and cockpit relocation.

### Requirement 10: Skill Rewrite and `specs/` Cleanup

**User Story:** As a maintainer, I want the retained skills neutralized of epic coupling and the orphaned `specs/` artifacts removed, so that every referenced path resolves on disk and the repository reflects the neutral grouping model.

#### Acceptance Criteria

1. WHEN the cleanup completes, THE Repository SHALL contain zero references to mandatory epics in any retained skill, such that each retained skill organizes around the neutral grouping model.
2. WHEN the cleanup completes, THE Repository SHALL make sure that every path referenced by a retained skill resolves on disk, with zero unresolved references remaining, the strict reading of refactor spec Property 5 and Requirement 10.7. (Property: no-dangling-skill-reference)
3. THE Repository SHALL count the approximately 20 references that dangle today toward the zero-unresolved-references end-state, not only references broken by this cleanup.
4. WHERE a retained skill example path names a removed directory, THE Repository SHALL reword the example path to a neutral placeholder, such that no example path resolves to a removed directory.
5. THE Repository SHALL remove the orphaned `specs/` artifacts: `verifications/`, `epics/`, `codebase-wiki/`, and the generated `adr-wiki/`, `epics-wiki/`, `skills-wiki/`, and `conventions-wiki/` directories.
6. THE Repository SHALL remove the JSON side-cars: `blind-spots`, `drift-report`, `skill-graph`, `receipts`, `rule-matrix`, `import-boundaries`, and `traceability-matrix`.
7. THE Repository SHALL remove the `*_LATEST.md` files, the process docs, the stray yaml files `agent-locks`, `planning-status`, and `tombstones`, and the `viz.html` file.
8. THE Repository SHALL remove the directories `benchmarks/`, `archive/`, `migrations/`, `security/`, `tech-architecture/`, `wayfinder/`, `workflows/`, `agent-guide/`, `templates/`, and `metrics/`.
9. THE Repository SHALL remove the `specs/product/` bigpowers content, and SHALL migrate the product concept into `.agent/`.
10. WHEN performing cleanup, THE Repository SHALL record each removal batch as a separate, individually-revertible git commit, consistent with refactor spec Requirement 10.6 and design section 9.3.
11. THE Repository SHALL NOT delete every artifact in one mass-delete commit.
12. IF a removal batch or the product migration fails, THEN THE Repository SHALL leave the tree in its pre-batch state and SHALL return an error indicating the failed batch.

### Requirement 11: Execution Artifact, Root AGENTS.md

**User Story:** As a maintainer, I want a repo-root `AGENTS.md` for the TrueNorth-MCP project itself, so that the project documents its own agent workspace once against the final layout.

#### Acceptance Criteria

1. WHEN this spec reaches execution against the final `.agent/` layout, THE Repository SHALL author a repo-root `AGENTS.md` for the TrueNorth-MCP project.
2. THE Repository SHALL wire the repo-root `AGENTS.md` to the final `.agent/` layout, and SHALL NOT wire it to `specs/`.

### Requirement 12: Out-of-Scope Boundaries

**User Story:** As a maintainer, I want the excluded work stated plainly, so that no one builds a metrics feature or a tracker integration under this spec.

#### Acceptance Criteria

1. THE Spec SHALL exclude workflow and cycle-time metrics as a named feature, and SHALL treat that feature as requiring more research.
2. THE Repository SHALL remove the `specs/metrics/` directory as retired instrumentation, and SHALL NOT build a metrics feature.
3. THE Spec SHALL keep the agent-cost telemetry at `.agent/telemetry/runs.yml` as a separate existing concept.
4. THE Spec SHALL exclude any live tracker sync, tracker API integration, network request, and stored tracker credential, consistent with Requirement 8.
5. THE Spec SHALL exclude user-defined custom methodology profiles, consistent with Requirement 3.
