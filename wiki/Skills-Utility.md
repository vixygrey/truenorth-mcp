# Skills: Utility and cross-phase

These skills do not belong to one lifecycle phase. They run before engagement, across
phases, or as standalone specialized operations: plan intake, large-effort mapping, stall
diagnosis, documentation and writing aids, reporting and dashboards, benchmarking, design
extraction, and server hardening.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

---

## Intake and orientation

### audit-plan

Evaluate an incoming project plan against the principles and conventions, and produce a
READY or NOT READY verdict.

- **What it does**: scores the plan through three lenses (principles alignment, conventions
  completeness, and a pre-flight of the build commands), closes gaps conversationally, and
  writes a plan audit report with a verdict.
- **When to use it**: when a new project arrives, when adapting a foreign plan, or before
  `seed-conventions` on an unfamiliar codebase.
- **Hard gate**: do not start build skills until the verdict is READY. A plan missing test
  commands, scope boundaries, or success criteria produces drift.
- **Handoff**: `survey-context` (READY), `seed-conventions` (needs bootstrapping),
  `elaborate-spec` (needs elaboration), `migrate-spec` (foreign format), or `grill-me`.

### find-way

Plan a large effort as a shared map of decision tickets on an issue tracker, resolved one
at a time.

- **What it does**: charts a wayfinder map (a single tracker issue) with child decision
  tickets (research, prototype, grilling, task), works them one at a time until the route
  is clear, and records each decision back on the map. It plans, it does not build.
- **When to use it**: when an idea is too big for one session, needs structured exploration
  before implementation, or requires mapping decisions before building.
- **Key rule**: never resolve more than one ticket per session (except research). Refer to
  every map and ticket by name.
- **Note**: this skill deliberately uses the issue tracker for a decision map, which is a
  scoped exception to the general convention that a skill does not create tracker issues.

### diagnose-stall

Diagnose why agent orchestration stopped producing progress.

- **What it does**: reads the state, checks for contention, inspects the background
  terminals, classifies the stall type (waiting_approval, blocked_dependency,
  agent_exhausted, misconfigured_loop, external_io, unknown), and recommends one recovery
  action.
- **When to use it**: when work appears hung, there is no output for several minutes, or a
  subagent never returned. It is invoked by `/loop`, `dispatch-agents`, and `execute-plan`.
- **Hard gate**: do not restart work blindly. Run the diagnostic first.
- **Handoff**: `survey-context` if the state is unclear, or resume the prior skill.

---

## Documentation and writing aids

### maintain-wiki

Keep the concept-wiki bundle consistent with its source docs.

- **What it does**: three operations. Ingest (regenerate concept pages from a source doc, a
  SKILL.md, or the conventions), lint (stale, orphan, missing cross-reference,
  contradiction, broken link), and query (search across the concept pages).
- **When to use it**: to keep the wiki current with the skills, the conventions, and the
  agent guide.
- **Hard gate**: none stated. The verify confirms no concept page is older than its source.

### simple-english

Write or rewrite technical text with the rules of ASD-STE100 Simplified Technical English.

- **What it does**: selects a mode (pragmatic or strict), classifies each passage as
  procedural or descriptive, fixes the vocabulary, applies the core limits (sentence length,
  approved modals, active voice), and runs a deterministic lint gate. It enforces the 53
  rules of the standard.
- **When to use it**: for documentation, a README, a runbook, an error message, release
  notes, or on "de-slop" and "make this readable".
- **Hard gates**: classify the text first. Never change code, identifiers, CLI flags, or
  quoted errors. Never claim STE compliance; final approval rests with the writer.

---

## Reporting and visualization

### visual-dashboard

Start a browser-based, read-only dashboard that visualizes the project status from the
cockpit.

- **What it does**: starts a local server that reads the cockpit files (state, release
  plan, execution status) and serves a read-only PM view and a JSON status endpoint. It
  prefers the `truenorth://` resources when driven from the MCP server.
- **When to use it**: to visualize the architecture, the plans, and the status.
- **Hard gate**: the dashboard is read-only. Do not use a visualization to make a decision
  without consulting the source data.

### generate-allure-report

Generate Allure-ready reports from the project YAML metadata.

- **What it does**: reads the execution status, release plan, task groups, task files, and
  bug registry, then produces a JUnit results file (one test case per story), a categories
  file, and an executor file in `allure-results/`.
- **When to use it**: when preparing a progress dashboard, integrating with Allure TestOps,
  or generating a CI report.
- **Handoff**: none. This is a terminal skill with no downstream step.

### run-benchmark

Run a skill quality benchmark with N-run, with-and-without-skill delta grading.

- **What it does**: reads a benchmark definition, runs each scenario N times with and
  without the skill loaded (isolating the skill's causal contribution as a delta), splits
  train from validation scenarios, and writes a pass@k report that `evolve-skill` consumes.
- **When to use it**: before and after `evolve-skill`, to prove a quality change is an
  improvement, not a regression.
- **Hard gate**: do not use benchmark scores to declare a skill good or bad in isolation.
  They measure relative quality versus a baseline; they catch regressions, they do not
  certify correctness. A negative validation delta blocks release.

---

## Specialized operations

### context7-mcp

Fetch current library docs through the Context7 MCP server instead of training data.

- **What it does**: resolves a library id, queries the docs (one concept per call), caches
  the result with a TTL, and answers from the fetched docs with a citation. It bounds itself
  to three calls per question.
- **When to use it**: on a setup, API, or code-example question for a specific library.
- **Hard gates**: at most three Context7 calls per question. Check the cache before an HTTP
  fetch. On a quota or rate-limit error, emit a `CONTEXT7_UNAVAILABLE` block rather than
  silently answering from training data.

### extract-design

Extract a `DESIGN.md` from an HTML prototype using a headless browser.

- **What it does**: launches a headless browser in a light and dark dual pass, collects the
  computed styles, classifies tokens (colors, typography, spacing, rounding, components),
  generates the design prose, and validates with a design-token linter.
- **When to use it**: when the user has an HTML prototype and wants a `DESIGN.md`, or right
  after a new project is scaffolded.
- **Hard gates**: the extraction must use a headless-browser dual pass; static-HTML tokens
  are invalid. Flag a low-confidence assertion. Do not ship without running the linter.
- **Handoff**: `grill-me` with the uncertain-decisions context.

### harden-vps

Harden a production Linux VPS across three independently verifiable layers.

- **What it does**: applies the OS layer first (UFW firewall, fail2ban, unattended-upgrades,
  SSH hardening), then the application layer (systemd hardening, monitoring alerts, backup
  automation), then the provider layer (health checks, backups, snapshots), with an
  eight-gate verification.
- **When to use it**: to secure a production server, harden a VPS, or audit server
  security.
- **Hard gate**: run `ufw status` first; no firewall means layer 1 takes priority over
  everything.
