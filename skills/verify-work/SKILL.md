---
name: verify-work
description: Multi-phase UAT gate. Cold-start smoke, build, typecheck, lint, tests, step-by-step manual verification, and a gaps-closure loop. Use it after execute-plan or develop-tdd, before audit-code.
---

# Verify Work

> **HARD GATE**: No story is done until manual UAT for the active story is confirmed with evidence.
>
> **HARD GATE**: Do NOT run on `main` or `master`. Use the feature branch from `kickoff-branch`.

Review answers "is the code good?". Verify answers "does the built thing do what
was promised?".

## Modes

- Default: full UAT plus the gaps loop.
- `--smoke`: cold-start only plus one happy-path flow. Use it for hotfixes.
- `--cli`: CLI-tool verification. Replaces cold-start with a binary smoke
  checklist. Use it for a CLI tool with no server process.

## Risk-scaled depth

Read the `risk:` field from the story (default `P1` when absent) to scale the rigor:

- **P0**: full multi-phase verify, plus `security-review` (step 5), plus the NFR evidence gate (step 5b).
- **P1**: standard verify (build, test, lint, step-by-step manual).
- **P2**: smoke, typecheck, lint only. Skip tests, the security scan, and step-by-step manual UAT.
- **P3**: typecheck and lint only. Skip smoke, tests, the security scan, and step-by-step manual UAT.

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
2. **Cold-start smoke** (skip if P3): stop the server, clear the caches, boot
   from scratch.
3. **Agent-guide preflight**: read the project agent guide for the build, test,
   and lint commands.
4. **Mechanical gates**: build, then typecheck, then lint, then tests. Skip tests
   if P2 or P3. Run each gate command through the `truenorth_verify_gate` tool, so
   the pass verdict comes from an exit-0 result, not from prose.

> **HARD GATE, one-test-minimum terminal verdict**: at least one mechanical gate MUST be a real terminal-verdict command that exits 0 or non-zero, not prose. Record that command's output from a single contiguous run as the `terminal_verdict` evidence. Do NOT merge evidence from multiple runs into one verdict. Each failing run gets its own evidence block.

5. **Security scan** (skip if P2 or P3): run `security-review` against the git
   diff (working tree versus merge-base). When any HIGH finding with confidence 8
   or more exists, block the gate. Record the findings. Allow a documented
   exception. A MEDIUM or LOW finding warns but does not block.
   5a. **Blind-spot check**: detect structural quality gaps beyond percentage
   coverage (verify-gap, test-gap, stale-tag). When any HIGH-severity finding
   exists, block the PASS gate. A MEDIUM or LOW finding warns but does not block.
   5a2. **Completeness critic**: run an adversarial gap-finding pass. Classify each
   finding as BLOCKER, WARNING, or FILLED. A BLOCKER aborts the merge gate. A
   WARNING enters the gaps loop. FILLED is evidence only.
   5b. **NFR evidence gate** (P0 only): produce a go or no-go on three dimensions,
   performance, reliability, and operability. Read the thresholds from the test
   plan. A FAIL on any dimension blocks the gate.
6. **Step-by-step UAT** (skip if P2 or P3): one user-observable action at a time.
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

## UAT dialogue

- Pass: the user confirms each step.
- Fail: capture expected versus actual. Do not mark the story done.

## Persist verification evidence

After UAT passes, write structured evidence for the story:

```yaml
story_id: e01s01
verified_at: '2026-06-11T14:30:00Z'
verifier: verify-work
phases:
  smoke:
    passed: true
  build:
    passed: true
    command: 'npm run build'
  typecheck:
    passed: true
  lint:
    passed: true
  tests:
    passed: true
    coverage: '94.2%'
  terminal_verdict:
    command: 'npm test'
    exit_code: 0
    captured_at: '2026-06-11T14:25:00Z'
    note: 'Single run. Do not merge output from other attempts.'
  manual:
    steps:
      - step: 'Open /login'
        expected: 'Login form renders'
        actual: 'Login form rendered correctly'
        passed: true
  gaps:
    closed: true
```

> **HARD GATE**: verification evidence MUST be persisted before you mark the story done. No evidence means not verified.

## --cli mode

For a CLI tool with no server process, use `--cli`. Detect the binary and run the
smoke checklist. See [REFERENCE.md](REFERENCE.md#cli-mode).

## Verify

Confirm that a verification-evidence file exists for the story. Run the project
verify command through the `truenorth_verify_gate` tool. A pass returns exit 0.

## Handoff

READY. Next: audit-code.
Writes: `state.yaml` `handoff.next_skill = audit-code`.
