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

- **What it does**: confirms the task name, selects the VCS procedure (git or Jujutsu),
  anchors on an updated clean default branch, creates an isolated worktree or branch, and
  runs preflight through `truenorth_verify_gate` to confirm green before any code.
- **When to use it**: when starting a feature or task.
- **Inputs**: the task name and the `vcs.kind` from the state.
- **Outputs**: a feature branch or worktree, a confirmed green baseline.
- **Hard gate**: no direct work on `main` or `master`. A red preflight blocks kickoff;
  route to `quick-fix` or `fix-bug`.
- **Handoff**: gate READY, next `develop-tdd` or `execute-plan`.

### develop-tdd

Test-driven development with a red-green-refactor loop over vertical slices.

- **What it does**: plans the behaviors, writes one failing test (RED), the minimal code to
  pass (GREEN), then an optional REFACTOR, driving each transition through
  `truenorth_tdd_cycle` and each verify through `truenorth_verify_gate`. It enforces a
  two-commit red/green policy and a per-cycle checklist.
- **When to use it**: for a feature (a group task) or a bug (a BUG report).
- **Inputs**: the active group story tasks or the BUG report, and any test plan.
- **Outputs**: tested code, two commits per behavior, a passing verify.
- **Hard gates**: not on `main`. No code before a plan. RED must fail before GREEN. Never
  refactor while RED. Never combine a test and a fix in one commit.
- **Handoff**: gate READY, next `verify-work`.

### build-epic

The task-group build cycle. Advances the build flow one step per invocation in resume
mode.

- **What it does**: orchestrates the eight-step build flow for one story: security-review,
  survey, plan, kickoff, TDD, verify, the non-optional audit-code gate, commit-message, and
  release-branch. It records timestamps and refreshes traceability.
- **When to use it**: for release work, instead of an ad-hoc `execute-plan`. It is called
  by `orchestrate-project`, scoped to one story, and is not a replacement for it.
- **Inputs**: the state, execution status, release plan, and the active task group.
- **Outputs**: an advanced build flow and an updated execution status.
- **Hard gates**: set `active_flow: build_group` and `active_group` first. Not on `main`
  before step 3. The audit-code gate at step 6 loops back to step 4 on a fail.
- **Modes**: default (resume one step), or `--fast` (coalesce read-and-report steps without
  skipping any checklist item).

### execute-plan

Batch-execute the active group tasks sequentially, with a human checkpoint after each step.

- **What it does**: reads the active group, then for each task announces it, executes,
  runs the verify (green before advancing), logs decisions, and checkpoints with the user.
  It spawns each skill with a fresh context, passing decisions only through the state
  handoff.
- **When to use it**: when the user has an approved plan and wants step-by-step oversight.
- **Inputs**: the state and the matching task group.
- **Outputs**: executed tasks with evidence, an updated execution status.
- **Hard gates**: not on `main`. The active group must exist with a runnable verify per
  task.
- **Handoff**: `verify-work`, `run-evals`, `audit-code`, `simulate-agents`,
  `commit-message`, `release-branch`.

### orchestrate-project

The meta-skill that coordinates a multi-phase project through the six-phase core loop with
hard gates.

- **What it does**: maintains the phase state, routes to the phase skill, applies
  methodology lenses, enforces the gates, gatekeeps between stories in the build phase
  (using `build-epic` per story), and pauses for confirmation between phases.
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

Set up a pre-commit hook with lint-staged (Prettier), type checking, and tests.

- **What it does**: detects the package manager, installs Husky, lint-staged, and Prettier,
  writes `.husky/pre-commit`, `.lintstagedrc`, and a Prettier config if missing, then
  verifies the hook runs.
- **When to use it**: when the user wants a pre-commit hook, Husky, lint-staged, or
  commit-time formatting, type checking, or testing.
- **Inputs**: the repo and its package manager.
- **Outputs**: an installed and verified pre-commit hook.
- **Hard gate**: the pre-commit and commit-msg hooks must run before a commit lands.
  Skipping a hook is forbidden unless explicitly authorized and documented.
- **Note**: this is a Node/Husky setup, distinct from `guard-git` (a harness pre-command
  hook) and the repo's own `.githooks`.

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

Set up a CI workflow with forge-neutral guidance and local validation.

- **What it does**: resolves the forge, detects the stack from the manifest, applies a
  test-build-release template (GitHub only), validates the YAML and permissions, and
  dry-runs it locally. It documents common CI failure patterns.
- **When to use it**: before the first merge to main on a supported forge.
- **Hard gate**: do not ship without CI on a supported forge. On an unsupported forge it
  writes nothing and reports honestly rather than claiming a gate it cannot run.
- **Related**: the CI counterpart of `wire-observability`.

### wire-observability

Add structured JSON logging, observability commands, and idempotent setup scripts.

- **What it does**: assesses the current logging, adds structured JSON logs at the
  boundaries, documents the health-check and metrics commands in the agent guide, and writes
  idempotent setup scripts.
- **When to use it**: when a project needs production-readiness instrumentation, or as a
  production-readiness gate at any phase. Recommended at the end of the first working slice.
- **Hard gate**: observability is not optional. Never log a secret or PII.

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
  smoke), detecting the build command from the manifest and the deploy target from
  environment variables (Vercel, Netlify, an MCP tool, rsync/SSH, or a custom command). It
  verifies three independent facts before declaring success.
- **When to use it**: from a CI/CD pipeline or post-merge on `main`, as the deploy half of
  CI/CD.
- **Hard gates**: run tests first. Never deploy from a feature branch. Chain
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
  the registry publish command, confirms the version appears, and surfaces actionable error
  hints. It prefers tag-driven publishing from CI.
- **When to use it**: to publish a package to a language registry.
- **Hard gates**: verify prerequisites first. Always run `--dry-run` first, because
  registries are append-only and a bad publish cannot be fully undone.

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

A streamlined fast path for a trivial data-only fix. No TDD, no branching ceremony.

- **What it does**: evaluates strict entry criteria (purely data, no logic, one file, five
  lines or fewer, single-assertion verify), applies the change, verifies, and commits with a
  `fix:` message that documents the skipped skills. It aborts to `investigate-bug` or
  `fix-bug` if any guardrail triggers.
- **When to use it**: for a trivial data-only fix (a missing key, a typo, a config value).
- **Hard gate**: all entry criteria must pass. Any guardrail (more than one file, more than
  five lines, a logic change, a complex verify, a test break) aborts immediately.
- **Handoff**: `release-branch`.

### craft-skill

Create a new skill with the correct structure, progressive disclosure, and bundled
resources.

- **What it does**: gathers requirements, verifies the skill principles (atomic verb-noun,
  deep, gated, verifiable), drafts the SKILL.md and a reference file, reviews with the user,
  and runs a completion-honesty gate that validates the name, description, body, and parse.
- **When to use it**: to create a new skill for the lifecycle.
- **Frontmatter discipline**: every skill declares `name`, `description`, and `kind`.
  `kind` is `prose` or `scripted`. A scripted skill also declares a runnable `verify`
  command. A non-verb-noun name needs a concise `name_exception`. Do not add model or
  effort metadata.
- **Description discipline**: 1024 characters max, third person, capability plus "Use it"
  triggers only. No workflow steps, phase chains, numbered lists, or HARD GATE prose in the
  description.
- **Hard gate**: validate the skill before merge. Scripted skills must execute their declared
  verify command; prose skills must pass structural validation.

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
