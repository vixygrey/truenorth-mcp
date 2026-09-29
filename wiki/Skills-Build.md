# Skills: Build

The Build phase turns a plan into working, tested code, then prepares it for release. It
is the largest phase. The skills group into the build loop, git safety, project setup, the
deploy and publish path, and a few specialized builders.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

---

## The build loop

### kickoff-branch

Create an isolated worktree or branch and verify a clean baseline before code.

- **What it does**: confirms the task name, anchors on an updated clean Git default
  branch, creates an isolated worktree or branch, and runs preflight through
  `truenorth_verify_gate` before any code.
- **When to use it**: when starting a feature or task.
- **Inputs**: the task name and Git repository state.
- **Outputs**: a feature branch or worktree, a confirmed green baseline.
- **Hard gate**: no direct work on `main` or `master`. A red preflight blocks kickoff;
  route to `quick-fix` or `fix-bug`.
- **Handoff**: gate READY, next `develop-tdd` or `execute-group`.

### develop-tdd

Test-driven development with a red-green-refactor loop over vertical slices.

- **What it does**: plans the behaviors, writes one failing test (RED), the minimal code to
  pass (GREEN), then an optional REFACTOR. It drives each transition through
  `truenorth_tdd_cycle` and each verify through `truenorth_verify_gate`. RED evidence
  stays local. Each verified test and implementation pair becomes one atomic green commit.
- **When to use it**: for a feature from a group task or a bug planned in an external issue.
- **Inputs**: the active group story tasks or the external issue's TDD plan.
- **Outputs**: tested code, one atomic green commit per behavior, a passing verify.
- **Hard gates**: not on `main`. No code before a plan. RED must fail before GREEN. Never
  commit or push a deliberately failing tree. Never refactor while RED.
- **Handoff**: gate READY, next `verify-work`.

### execute-group

The profile-aware execution conductor for an active task or group.

- **What it does**: owns the selected execution scope from threat modeling and branch
  kickoff through TDD, task verification, behavioral verification, traceability, audit,
  review, commit, and release. It may delegate disjoint waves but retains ordering, state,
  and verification ownership.
- **When to use it**: when approved work is ready to build, directly or from
  `orchestrate-project` during Execute.
- **Inputs**: the profile, state, execution status, release plan, and selected work-item
  ledger. Group artifacts are required only when the profile groups work.
- **Outputs**: verified code, updated execution status and handoff, and a release-ready
  branch when every gate passes.
- **Hard gates**: set `active_flow: execute_group`; do not implement on `main` or
  `master`; require a runnable verify command per task; block on failed security,
  verification, traceability, audit, review, or release gates.
- **Modes**: `checkpoint` confirms scope and pauses after each verified work item.
  `autonomous` continues without routine prompts. Both modes stop at failures,
  unresolved blockers, scope changes, external decisions, and release safety gates.
- **Profile behavior**: grouped profiles use their active epic, milestone, or optional
  ticket group. Ungrouped profiles use `active_task` and omit group fields.

### orchestrate-project

The meta-skill that coordinates a multi-phase project through the six-phase core loop with
hard gates.

- **What it does**: maintains the phase state, routes to the phase skill, applies
  methodology lenses, enforces the gates, and calls `execute-group` as the sole
  conductor during Execute. It pauses for confirmation between phases.
- **When to use it**: to coordinate complex, multi-stage work. A single-skill task uses the
  dedicated skill instead.
- **Inputs**: the state (`project_cycle`), the cockpit files.
- **Outputs**: an advanced project cycle with per-phase artifacts.
- **Modes**: standard (all gates), fast-track (skip negotiable gates), ad-hoc (warnings
  only).
- **Note**: this skill is the reference for the phase loop, though its phase names lag the
  runtime's canonical set (see the terminology note on [The skill workflow](The-skill-workflow)).

---

## Git safety

### guard-git

Block a dangerous git command before an agent runs it, and enforce opt-in policy.

- **What it does**: installs a pre-command hook that always blocks the dangerous patterns
  (force push, `reset --hard`, `clean`, `branch -D`, `checkout` or `restore` of a path).
  Opt-in flags add branch protection (block a commit or push to `main`/`master`) and
  Conventional Commits enforcement. A separate pre-commit hook scans staged diffs for
  secrets.
- **When to use it**: when the user wants git-safety hooks, or to mirror the same policy
  across coding tools.
- **Inputs**: the harness hook config.
- **Outputs**: an installed hook, a blocked or allowed command.
- **Modes**: `GIT_GUARDRAILS_MODE` selects the harness contract (claude/cursor stderr and
  exit 2, or gemini JSON decision). Opt-in flags: `GIT_GUARDRAILS_PROTECT_BRANCH`,
  `GIT_GUARDRAILS_LAND`, `GIT_GUARDRAILS_CONVENTIONAL`.

### hook-commits

Set up pre-commit hooks for Node.js projects with Husky, lint-staged (Prettier), type checking, and tests.

- **What it does**: detects the package manager, installs Husky, lint-staged, and Prettier,
  writes `.husky/pre-commit`, `.lintstagedrc`, and a Prettier config if missing, then
  verifies the hook runs.
- **When to use it**: when the user wants a pre-commit hook, Husky, lint-staged, or
  commit-time formatting, type checking, or testing in a Node.js / JavaScript / TypeScript project.
- **Inputs**: the repo and its package manager.
- **Outputs**: an installed and verified pre-commit hook.
- **Hard gate**: the pre-commit and commit-msg hooks must run before a commit lands.
  Skipping a hook is forbidden unless explicitly authorized and documented.
- **Note**: this is a Node/Husky setup, distinct from `guard-git` (a harness pre-command
  hook) and the repo's own `.githooks`. Non-Node projects use native `.githooks` or language-native tooling without installing Node dependencies.

---

## Project setup and production readiness

### setup-environment

Pre-install dependencies and configure tools before development begins.

- **What it does**: reads the agent guide for required runtimes, verifies versions, installs
  dependencies from a lockfile, copies `.env.example`, runs a smoke check, and records the
  versions in the state.
- **When to use it**: at session start on a fresh clone, before `kickoff-branch`, or on
  "setup environment".
- **Hard gate**: setup must be idempotent and reproducible, with a clear error and
  remediation on failure.

### wire-ci

Set up a GitHub Actions CI workflow with local validation.

- **What it does**: detects the project stack from the manifest, applies a
  GitHub test-build-release template (`.github/workflows/`), validates the YAML and permissions, and
  dry-runs it locally. It documents common CI failure patterns.
- **When to use it**: before the first merge to main on GitHub.
- **Hard gate**: do not ship without CI on supported forges (GitHub). On an unsupported forge
  (GitLab, Bitbucket, Codeberg, Gitea), it writes nothing and reports honestly rather than claiming a gate it cannot run.
- **Related**: the CI counterpart of `wire-observability`.

### wire-observability

Add structured JSON logging, observability commands, and setup scripts scaled to system risk.

- **What it does**: assesses current logging, adds structured JSON logs at network
  boundaries for services/APIs, documents health-check and metrics commands in the agent guide, and writes
  idempotent setup scripts.
- **When to use it**: when a project needs production-readiness instrumentation, or as a
  readiness gate before deploying services.
- **Hard gate**: observability requirements scale to deployment profile. Network services and APIs
  require structured JSON logging and health checks; CLI tools and libraries require human-readable streams and proper exit codes. Never log secrets or PII.

### validate-contracts

Assert data-shape consistency across system boundaries.

- **What it does**: validates a contract file in `specs/contracts/` in one of three modes:
  schema (an API response against a JSON schema), key-set (a missing or unexpected key
  across two sources), or shape (a column type or format). It names the divergence and exits
  non-zero on failure.
- **When to use it**: before a deploy or migration, after an API change, or on translation
  and config files.
- **Hard gates**: do not deploy or migrate without running it. A contract file must be
  version-controlled; a contract unreviewed for 30 days is flagged stale.
- **Verify arc**: part of the verify sequence, after `verify-work` and before `smoke-test`.

---

## Deploy and publish

### deploy

Build, verify the artifact, deploy, wait, then smoke the deployment.

- **What it does**: runs a five-stage pipeline (build, verify artifact, deploy, wait/retry,
  smoke), detecting the build command from the manifest and using explicit `DEPLOY_TARGET` configuration
  (vercel, netlify, mcp, rsync, custom). It verifies three independent facts before declaring success.
- **When to use it**: from a CI/CD pipeline or post-merge on `main`, as the deploy half of
  CI/CD.
- **Hard gates**: run tests first. Never deploy from a feature branch. Explicit approval is required
  before production deployment. Never pass tokens on the command line; redact secrets from logs. Chain
  `deploy` then `smoke-test`.

### smoke-test

Post-deploy health check against a live URL.

- **What it does**: reads `smoke-checks.yaml`, runs each HTTP check (status, an optional
  body signal, an optional response-time threshold), asserts the results, and captures a
  report as release evidence.
- **When to use it**: standalone, or as the final step of `deploy`.
- **Hard gates**: do not run against a URL that is not deployed yet. A failed smoke check
  means the deploy is broken; do not mark it successful.

### publish-package

Package-registry publishing for npm, crates.io, PyPI, and Homebrew.

- **What it does**: detects the package type from the manifest, verifies prerequisites, runs
  the dry-run check, prompts for explicit approval, runs the registry publish command, confirms the version appears, and surfaces actionable error
  hints. It prefers tag-driven publishing from CI.
- **When to use it**: to publish a package to a language registry.
- **Hard gates**: verify prerequisites first. Always run `--dry-run` first, and require explicit human
  confirmation before the live publish, because registries are append-only. Never expose raw tokens on the CLI.

---

## Specialized builders

### spike-prototype

A throw-away prototype for an unknown problem space. The output is learning, not shipping
code.

- **What it does**: states the specific question, sets a timebox, writes the simplest code
  that answers it (ignoring error handling, tests, and quality), writes a spike note with
  the findings and implications, then deletes the spike code.
- **When to use it**: when the technology or approach is unexplored, an estimate is
  impossible without experimentation, or the user says "spike" or "proof of concept".
- **Hard gate**: spikes are time-boxed experiments, not shipping code. Do not merge a spike
  without a plan to replace it.
- **Handoff**: the spike note feeds `plan-work`.

### quick-fix

A streamlined path for a trivial data-only fix on a small isolated Git branch.

- **What it does**: runs `kickoff-branch`, evaluates strict entry criteria (purely data,
  no logic, one file, five lines or fewer, single-assertion verify), applies the change,
  verifies, and commits with a `fix:` message that documents the skipped analysis skills.
- **When to use it**: for a trivial data-only fix such as a missing key, typo, or config
  value.
- **Hard gate**: never mutate the default branch directly. Every entry criterion must
  pass. A larger diff, logic change, complex verify, or test break aborts to the full bug
  workflow.

- **Handoff**: `release-branch`.

### craft-skill

Create a new skill with the correct structure, progressive disclosure, and bundled
resources.

- **What it does**: gathers requirements, selects the prose or scripted template, drafts
  the SKILL.md and supporting files, reviews with the user, and validates the result.
- **When to use it**: to create a new skill for the lifecycle.
- **Frontmatter discipline**: every skill declares `name`, `description`, and `kind`.
  `kind` is `prose` or `scripted`. A scripted skill also declares a nonempty `verify`
  command. A non-verb-noun name needs a concise `name_exception`. Do not add model or
  effort metadata.
- **Description discipline**: 1024 characters max, third person, capability plus "Use it"
  triggers only. No workflow steps, phase chains, numbered lists, or HARD GATE prose in the
  description.
- **Size discipline**: the complete SKILL.md, including frontmatter, is 150 lines or fewer.
- **Hard gate**: `validate_skill` must pass before merge. Scripted skills must also execute
  their declared `verify` command; prose skills need no synthetic command.

### align-grid

Build an editorial webpage on a genuine Müller-Brockmann modular grid, and prove it with a
harness.

- **What it does**: encodes the International Typographic Style discipline (columns,
  modules, an 8px baseline, grotesque type, a restrained palette) plus the front-end
  engineering to make the grid load-bearing (one CSS-variable source of truth, a grid-toggle
  overlay, subgrid bands, a baseline lock, runtime optical alignment). It ships a scaffold
  generator (`grid_tokens.py`) and a verification harness (`verify_grid.js`).
- **When to use it**: to build a grid-disciplined editorial or report page.
- **Hard gate**: do not ship a grid page without running `verify_grid.js`. Zero failing
  assertions is the only passing bar, because a visually plausible layout is not evidence.
