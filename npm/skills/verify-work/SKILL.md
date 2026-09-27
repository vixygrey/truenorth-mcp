---
name: verify-work
description: "Multi-phase UAT gate with cold-start smoke, mechanical checks, manual verification, and a gaps loop. Supports fresh-context user simulation for gap discovery. Use it after execute-plan or develop-tdd, before audit-code, or for a mock-user or user-observable simulation."
kind: prose
---

# Verify Work

> **HARD GATE**: No story is done until at least one story-specific observable behavior smoke passes and its evidence is recorded.
>
> **HARD GATE**: Do NOT run on `main` or `master`. Use the feature branch from `kickoff-branch`.

Review answers "is the code good?". Verify answers "does the built thing do what
was promised?".

## Modes

- Default: risk-scaled verification plus the gaps loop.
- `--smoke`: cold-start only plus one happy-path flow. Use it for hotfixes.
- `--cli`: CLI-tool verification. Replaces cold-start with a binary smoke
  checklist. Use it for a CLI tool with no server process.
- `--simulate-user`: follow the Verification Script in a fresh context from the
  user's perspective. Record UX and behavior gaps. This mode does not replace
  mechanical gates or any manual confirmation required by the acceptance criteria.

## Risk-scaled depth

Read the `risk:` field from the story (default `P1` when absent). Every tier
exercises the changed behavior; risk changes the breadth and depth of evidence:

- **P0**: observable behavior smoke, configured mechanical gates, full UAT,
  `security-review` (step 5), broad regression coverage, and the NFR evidence
  gate (step 5b).
- **P1**: observable behavior smoke, relevant tests, configured mechanical
  gates, and standard UAT.
- **P2**: observable behavior smoke, targeted verification, and the relevant
  static gates.
- **P3**: minimal changed-surface behavior smoke and only the relevant static
  gates. For documentation or configuration, render, parse, or load the changed
  consumer surface.

## Process

0. **Branch check**: the branch must not be `main` or `master`.
   0a. **Preflight, CI green** (HARD GATE): when a PR is open, confirm CI is green
   before any phase. A failure blocks all phases. Route to `quick-fix` or
   `fix-bug`.
1. Read the active story tasks and the story spec for the capsule. Note the
   `risk:` level (P0 to P3).
   1a. **Pre-UAT verify validation**: for each task's `verify:` command, run it and
   detect a pattern mismatch before UAT begins. When a check fails, decide
   whether the pattern is wrong or the failure is genuine. Report the nearest
   match and confirm a verify-command update before you proceed. A mismatched
   verify command produces a false failure during UAT.
2. **Observable behavior smoke**: exercise at least one story-specific changed
   behavior and record expected versus actual results. For a server, include a
   cold start when stale state is a plausible risk. For a CLI, use `--cli`. For
   documentation or configuration, render, parse, or load the changed consumer
   surface.
3. **Agent-guide preflight**: read the project agent guide for the build, test,
   and lint commands.
4. **Mechanical gates**: run the configured gates relevant to the change through
   `truenorth_verify_gate`, so the pass verdict comes from an exit-0 result, not
   from prose. P0 and P1 run the full configured set. P2 runs targeted tests and
   relevant static gates. P3 runs only relevant static gates in addition to its
   mandatory behavior smoke.

> **HARD GATE, terminal verdict**: at least one gate MUST be a real terminal-verdict command that exits 0 or non-zero, not prose. Record that command's output from a single contiguous run as the `terminal_verdict` evidence. Do NOT merge evidence from multiple runs into one verdict. Each failing run gets its own evidence block.

5. **Security scan**: run `security-review` against the git diff (working tree
   versus merge-base) for P0 and for any security-sensitive change. A confirmed
   unresolved HIGH or CRITICAL finding blocks the gate. A `Needs investigation`
   item with potentially HIGH or CRITICAL impact requires explicit disposition
   before the gate passes. Record all findings and documented exceptions.
   5a. **Blind-spot check**: detect structural quality gaps beyond percentage
   coverage (verify-gap, test-gap, stale-tag). When any HIGH-severity finding
   exists, block the PASS gate. A MEDIUM or LOW finding warns but does not block.
   5a2. **Completeness critic**: run an adversarial gap-finding pass. Classify each
   finding as BLOCKER, WARNING, or FILLED. A BLOCKER aborts the merge gate. A
   WARNING enters the gaps loop. FILLED is evidence only.
   5b. **NFR evidence gate** (P0 only): produce a go or no-go on three dimensions,
   performance, reliability, and operability. Read the thresholds from the test
   plan. A FAIL on any dimension blocks the gate.
6. **UAT depth**: P0 gets full UAT and P1 gets standard UAT. P2 and P3 use their
   mandatory behavior smoke unless the acceptance criteria require human
   judgment. In `--simulate-user` mode, use a fresh context and record expected
   versus actual behavior. Require real user confirmation only for criteria that
   cannot be decided from observable automated evidence.
7. **Gaps loop**: a failure logs a gap, routes to `plan-work`, then re-verifies.
   An unaddressed HIGH finding from step 5 feeds this loop.
   7a. **Validation gate**: every task is `status: passing`, the evidence is
   recorded, and the execution status is updated.
   7b. **Reopen, do not refile**: a regression reopens the existing story or bug. Do
   not create a duplicate capsule entry.

## Verify sub-operations

### Cold-start smoke

Stop the server, clear the caches, and boot from scratch. Confirm that no stale
config affects the behavior.

### Gaps loop

After UAT, close any gap between the promised behavior and the actual behavior.

- Capture what the story task promised.
- Record what actually happened, expected versus actual.
- When the behavior does not match the promise, log the gap.
- Route back to `plan-work` or `develop-tdd` to fix the gap.
- Re-verify until the gap count is zero.

## Persist verification evidence

After verification passes, write structured evidence for the story:

Persist `story_id`, `verified_at`, `risk`, and phase evidence. The phase data
MUST include:

- `behavior_smoke`: command or action, expected result, actual result, and pass;
- `terminal_verdict`: command, exit code, and capture time from one run;
- each risk-required mechanical, security, UAT, and NFR result;
- `manual.required` with a reason and steps when human judgment is required; and
- gap status.

See [REFERENCE.md](REFERENCE.md#evidence-template) for the full shape.

> **HARD GATE**: verification evidence, including the behavior smoke, MUST be persisted before you mark the story done. No evidence means not verified.

## --cli mode

For a CLI tool with no server process, use `--cli`. Detect the binary and run the
smoke checklist. See [REFERENCE.md](REFERENCE.md#cli-mode).

## Verify

Confirm that a verification-evidence file exists for the story. Run the project
verify command through the `truenorth_verify_gate` tool. A pass returns exit 0.

## Handoff

READY. Next: audit-code.
Writes: `state.yaml` `handoff.next_skill = audit-code`.
