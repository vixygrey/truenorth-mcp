# Skill index

Every skill, alphabetical, with a one-line purpose and its phase page. The runtime serves
80 skills. Open a phase page for the deep per-skill entry (what it does, when to use it,
inputs, outputs, hard gates, and handoffs).

For the skill-to-skill flow, see [The skill workflow](The-skill-workflow).

| Skill                    | Purpose                                                         | Phase page                  |
| ------------------------ | --------------------------------------------------------------- | --------------------------- |
| `align-grid`             | Build an editorial webpage on a verified Müller-Brockmann grid  | [Build](Skills-Build)       |
| `assess-impact`          | Analyze the blast radius of a change before code                | [Plan](Skills-Plan)         |
| `audit-code`             | Self-review checklist before dispatching a reviewer             | [Verify](Skills-Verify)     |
| `audit-plan`             | Evaluate an incoming plan, produce a READY verdict              | [Utility](Skills-Utility)   |
| `build-epic`             | The task-group build cycle, one step per invocation             | [Build](Skills-Build)       |
| `change-request`         | Add a requirement or reorder task groups by WSJF                | [Plan](Skills-Plan)         |
| `commit-message`         | Draft a Conventional Commits message and its SemVer bump        | [Release](Skills-Release)   |
| `compose-workflow`       | Chain multiple skills into a custom workflow recipe             | [Sustain](Skills-Sustain)   |
| `context7-mcp`           | Fetch current library docs through the Context7 server          | [Utility](Skills-Utility)   |
| `craft-skill`            | Create a new skill with the correct structure                   | [Build](Skills-Build)       |
| `deepen-architecture`    | Find deepening opportunities in a codebase                      | [Design](Skills-Design)     |
| `define-language`        | Extract a ubiquitous-language glossary from the conversation    | [Design](Skills-Design)     |
| `delegate-task`          | Delegate one complex task to a subagent with two-stage review   | [Sustain](Skills-Sustain)   |
| `deploy`                 | Build, verify, deploy, wait, then smoke the deployment          | [Build](Skills-Build)       |
| `design-interface`       | Generate several interface designs, then compare                | [Design](Skills-Design)     |
| `develop-tdd`            | Test-driven development with a red-green-refactor loop          | [Build](Skills-Build)       |
| `diagnose-root`          | Four-phase root-cause analysis                                  | [Verify](Skills-Verify)     |
| `diagnose-stall`         | Diagnose why agent orchestration stopped                        | [Utility](Skills-Utility)   |
| `dispatch-agents`        | Dispatch multiple subagents in parallel                         | [Sustain](Skills-Sustain)   |
| `edit-document`          | Edit and improve an existing document                           | [Sustain](Skills-Sustain)   |
| `elaborate-spec`         | Refine a rough idea into a clear specification                  | [Discover](Skills-Discover) |
| `enforce-first`          | Apply the F.I.R.S.T test-quality rubric                         | [Verify](Skills-Verify)     |
| `evolve-skill`           | Benchmark-gated skill evolution                                 | [Sustain](Skills-Sustain)   |
| `execute-plan`           | Batch-execute the active group tasks with checkpoints           | [Build](Skills-Build)       |
| `extract-design`         | Extract a DESIGN.md from an HTML prototype                      | [Utility](Skills-Utility)   |
| `find-way`               | Map a large effort as decision tickets on a tracker             | [Utility](Skills-Utility)   |
| `fix-bug`                | Orchestrate the bug-fix chain                                   | [Verify](Skills-Verify)     |
| `gate-trace`             | Deterministic traceability quality gate                         | [Verify](Skills-Verify)     |
| `generate-allure-report` | Generate Allure-ready reports from the project metadata         | [Utility](Skills-Utility)   |
| `grill-me`               | Stress-test a plan through relentless questioning               | [Design](Skills-Design)     |
| `grill-with-docs`        | The doc-grounded variant of grill-me                            | [Design](Skills-Design)     |
| `guard-git`              | Block a dangerous git command, enforce opt-in policy            | [Build](Skills-Build)       |
| `harden-vps`             | Harden a production Linux VPS across three layers               | [Utility](Skills-Utility)   |
| `hook-commits`           | Set up a pre-commit hook with lint-staged                       | [Build](Skills-Build)       |
| `inspect-quality`        | An interactive QA session that logs bugs to the registry        | [Verify](Skills-Verify)     |
| `investigate-bug`        | Investigate a bug, find the root cause, write a fix plan        | [Verify](Skills-Verify)     |
| `kickoff-branch`         | Create an isolated worktree and verify a clean baseline         | [Build](Skills-Build)       |
| `maintain-wiki`          | Keep the concept-wiki bundle consistent with its sources        | [Utility](Skills-Utility)   |
| `map-codebase`           | Derive the tech-stack note by scanning the codebase             | [Discover](Skills-Discover) |
| `migrate-spec`           | Transform a foreign spec artifact into the project layout       | [Sustain](Skills-Sustain)   |
| `model-domain`           | Stress-test a plan against the domain model, capture invariants | [Design](Skills-Design)     |
| `orchestrate-project`    | Coordinate a multi-phase project with hard gates                | [Build](Skills-Build)       |
| `organize-workspace`     | Scan for disposable artifacts, propose a safe tidy              | [Sustain](Skills-Sustain)   |
| `plan-refactor`          | Create a refactor plan of tiny commits                          | [Plan](Skills-Plan)         |
| `plan-release`           | Sequence task groups into the release plan by WSJF              | [Plan](Skills-Plan)         |
| `plan-tests`             | Design a risk-scaled test architecture for a group              | [Plan](Skills-Plan)         |
| `plan-work`              | Write detailed, verifiable implementation tasks                 | [Plan](Skills-Plan)         |
| `publish-package`        | Publish a package to npm, crates.io, PyPI, or Homebrew          | [Build](Skills-Build)       |
| `quick-fix`              | A fast path for a trivial data-only fix                         | [Build](Skills-Build)       |
| `release-branch`         | Verify the gates and land a finished branch                     | [Release](Skills-Release)   |
| `request-review`         | Dispatch fresh reviewer agents with a dual-blind gate           | [Verify](Skills-Verify)     |
| `research-first`         | Search for prior art before implementing                        | [Discover](Skills-Discover) |
| `reset-baseline`         | Restore the project to a known clean state                      | [Sustain](Skills-Sustain)   |
| `respond-review`         | Act on reviewer feedback systematically                         | [Verify](Skills-Verify)     |
| `run-benchmark`          | Run a skill quality benchmark with delta grading                | [Utility](Skills-Utility)   |
| `run-evals`              | Eval-driven development, define evals before building           | [Verify](Skills-Verify)     |
| `run-planning`           | The discover-phase advancer                                     | [Plan](Skills-Plan)         |
| `scope-work`             | Define what is in and out of scope (spine step 1)               | [Plan](Skills-Plan)         |
| `search-skills`          | Find the right skill from a natural-language intent             | [Discover](Skills-Discover) |
| `security-review`        | Security analysis of code changes across files                  | [Verify](Skills-Verify)     |
| `seed-conventions`       | Generate the agent guide and conventions for a new project      | [Plan](Skills-Plan)         |
| `session-state`          | Track decisions and progress to prevent context rot             | [Sustain](Skills-Sustain)   |
| `setup-environment`      | Pre-install dependencies and configure tools                    | [Build](Skills-Build)       |
| `simple-english`         | Write technical text with Simplified Technical English          | [Utility](Skills-Utility)   |
| `simulate-agents`        | Run a mock-user and auditor agent before human review           | [Sustain](Skills-Sustain)   |
| `slice-tasks`            | Break a scoped PRD into vertical slices (spine step 2)          | [Plan](Skills-Plan)         |
| `smoke-test`             | Post-deploy health check against a live URL                     | [Build](Skills-Build)       |
| `spike-prototype`        | A throw-away prototype for an unknown problem space             | [Build](Skills-Build)       |
| `stocktake-skills`       | A batch audit of the skill catalog                              | [Sustain](Skills-Sustain)   |
| `survey-context`         | Read the current state, map the phase, suggest the next skill   | [Discover](Skills-Discover) |
| `terse-mode`             | An ultra-compressed communication mode                          | [Sustain](Skills-Sustain)   |
| `trace-requirement`      | Link story ids to the implementing code and tests               | [Verify](Skills-Verify)     |
| `using-truenorth`        | The one-time bootstrap and routing entry point                  | [Discover](Skills-Discover) |
| `validate-contracts`     | Assert data-shape consistency across boundaries                 | [Build](Skills-Build)       |
| `validate-fix`           | Prove a fix works and harden against recurrence                 | [Verify](Skills-Verify)     |
| `verify-work`            | The multi-phase UAT gate                                        | [Verify](Skills-Verify)     |
| `visual-dashboard`       | A read-only browser dashboard of the project status             | [Utility](Skills-Utility)   |
| `wire-ci`                | Set up a CI workflow with forge-neutral guidance                | [Build](Skills-Build)       |
| `wire-observability`     | Add structured logging and observability commands               | [Build](Skills-Build)       |
| `write-document`         | Write a high-integrity document using BMAD                      | [Sustain](Skills-Sustain)   |
