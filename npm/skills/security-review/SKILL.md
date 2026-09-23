---
name: security-review
description: 'Security analysis of code changes. Traces data flow and detects injection, auth bypass, secrets exposure, and unsafe deserialization across files. Use it when reviewing pending changes, before release-branch, during verify-work, during build-epic threat modeling, or when the user says "security review".'
kind: prose
---

# Security Review

> **HARD GATE**: requires git context (a branch with a merge-base or a diff). Writes only the security review report. A finding below confidence 8 of 10 is suppressed.

## Parallel mode

When running alongside `audit-code`, use isolated git worktrees so the scans do
not race on the same index. Each check gets a detached worktree, and the reports
still write only to the security review report.

## Five-phase scan

| #   | Phase                        | What                                                                                                                                                  |
| --- | ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Scope resolution**         | Detect the diff (the working tree versus the merge-base) through the git-context tool. Resolve the languages and frameworks from the dependency files |
| 2   | **Context research**         | Identify the existing security patterns, sanitization, and auth model in the codebase                                                                 |
| 3   | **Vulnerability assessment** | Trace user input to a sink. Check the auth boundaries, crypto, deserialization, and path operations                                                   |
| 4   | **False-positive filtering** | Cross-check each finding against the exclusion rules. Reject a confidence below 8                                                                     |
| 5   | **Report generation**        | Output structured markdown: file and line, severity, category, exploit scenario, fix                                                                  |

## Categories

Covered: SQLi, XSS, SSRF, command injection, auth bypass, unsafe deserialization, path traversal, IDOR, crypto flaws, secrets exposure, template injection, NoSQLi

## CWE mapping mandate (e45s26)

Every **new detection rule** added to this skill MUST:

1. Map to a [CWE](https://cwe.mitre.org/) ID in `REFERENCE-vuln-categories.md` (e.g. SQLi → CWE-89, XSS → CWE-79).
2. Ship **two fixture pairs** under `skills/security-review/fixtures/`:
   - **Positive** — minimal code the rule MUST flag (vulnerable pattern present).
   - **Negative** — structurally similar code the rule MUST NOT flag (safe pattern / false-positive guard).

| Rule                          | CWE     | Positive fixture                            | Negative fixture                            |
| ----------------------------- | ------- | ------------------------------------------- | ------------------------------------------- |
| SQL injection                 | CWE-89  | `fixtures/CWE-89-sqli-positive.py`          | `fixtures/CWE-89-sqli-negative.py`          |
| XSS (DOM)                     | CWE-79  | `fixtures/CWE-79-xss-positive.js`           | `fixtures/CWE-79-xss-negative.js`           |
| Missing tenant scoping (IDOR) | CWE-639 | `fixtures/CWE-639-idor-positive.go`         | `fixtures/CWE-639-idor-negative.go`         |
| Fail-open verify directive    | CWE-754 | `fixtures/CWE-fail-open-verify-positive.sh` | `fixtures/CWE-fail-open-verify-negative.sh` |

Before merging a new category, run both fixtures through the detection guidance and confirm positive flags / negative passes.

## SQL-safety doctrine (e45s41 — proven authorship)

Formal rule for SQL injection classification:

| SQL source                                                  | Attacker-reachable input?            | Verdict                         |
| ----------------------------------------------------------- | ------------------------------------ | ------------------------------- |
| Hardcoded / compile-time constant string                    | N/A                                  | **Safe** — proven authorship    |
| Developer-authored query with bound parameters only         | No dynamic fragments from user input | **Safe**                        |
| String concatenation / template with user-controlled values | Yes                                  | **Unsafe** — report as SQLi     |
| ORM query builder with user input in WHERE/JOIN             | Yes                                  | **Unsafe** unless parameterized |
| Stored procedure call with bound args                       | Args from trusted constants only     | **Safe**                        |
| Stored procedure with dynamic SQL inside                    | User input reaches EXEC              | **Unsafe**                      |

**Provenance test:** If the agent cannot prove the query string was authored entirely by the developer (no attacker-reachable interpolation), treat as vulnerable. Hardcoded SQL in migrations, seeds, and admin scripts is safe; anything reachable from HTTP/CLI/user input is not.

## Integration points

| Skill             | Touchpoint                                                                 |
| ----------------- | -------------------------------------------------------------------------- |
| `build-epic`      | Step 0 — threat-model group scope → the security review report             |
| `plan-work`       | `security:` field (none/low/medium/high) on story tasks                    |
| `plan-release`    | +2 WSJF risk boost for HIGH+ risk task groups                              |
| `audit-code`      | Checklist: "diff scanned — no unaddressed HIGH findings"                   |
| `request-review`  | Inject threat model categories + false-positive rules into reviewer prompt |
| `investigate-bug` | Security-impact assessment in RCA (NONE→CRITICAL)                          |
| `validate-fix`    | Recurrence hardening check for security bugs                               |
| `verify-work`     | Phase 5 — blocks on HIGH findings ≥ 8 confidence                           |
| `release-branch`  | Hard gate — blocks merge if unresolved HIGH findings                       |

## Report format

Each finding: **`File:Line` — Severity — Category**

- Description: how the vulnerability manifests
- Exploit scenario: concrete attack path
- Recommendation: fix with code example

## Reference files

- [Vuln categories](REFERENCE-vuln-categories.md) — detection guidance per vuln type
- [False positives](REFERENCE-false-positives.md) — hard exclusions + precedent
- [Confidence rubric](REFERENCE-confidence-rubric.md) — scoring methodology (0–10)

## Verify

Confirm the security review report exists, each detection rule has its positive
and negative fixture pair, and the git context resolves.
