---
name: security-review
description: 'Security analysis of code changes. Traces data flow and detects injection, auth bypass, secrets exposure, and unsafe deserialization across files. Use it when reviewing pending changes, before release-branch, during verify-work, during execute-group threat modeling, or when the user says "security review".'
kind: prose
---

# Security Review

> **HARD GATE**: requires git context (a branch with a merge-base or a diff). Writes only the security review report. Proven false positives may be excluded; an uncertain path with potentially HIGH or CRITICAL impact MUST be reported under `Needs investigation`.

## Parallel mode

When running alongside `audit-code`, use isolated git worktrees so the scans do
not race on the same index. Each check gets a detached worktree, and the reports
still write only to the security review report.

## Five-phase scan

| #   | Phase                        | What                                                                                                                                           |
| --- | ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Scope resolution**         | Detect the diff through the git-context tool. Resolve languages and frameworks from dependency files.                                          |
| 2   | **Context research**         | Identify existing security patterns, sanitization, trust boundaries, and the auth model.                                                       |
| 3   | **Vulnerability assessment** | Trace user input to sinks. Check auth boundaries, crypto, deserialization, and path operations.                                                |
| 4   | **Evidence classification**  | Apply proven exclusions. Classify supported vulnerabilities as confirmed and uncertain potentially high-impact paths as needing investigation. |
| 5   | **Report generation**        | Report confirmed findings, needs-investigation items, and exclusions separately.                                                               |

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

| Skill             | Touchpoint                                                                                                          |
| ----------------- | ------------------------------------------------------------------------------------------------------------------- |
| `execute-group`   | Threat-model the selected scope → the security review report                                                        |
| `plan-work`       | `security:` field (none/low/medium/high) on tasks; feed risk into the selected prioritization policy when supported |
| `audit-code`      | Checklist: "diff scanned — no unaddressed HIGH findings"                                                            |
| `request-review`  | Inject threat model categories + false-positive rules into reviewer prompt                                          |
| `investigate-bug` | Security-impact assessment in RCA (NONE→CRITICAL)                                                                   |
| `validate-fix`    | Recurrence hardening check for security bugs                                                                        |
| `verify-work`     | Phase 5 — blocks on unresolved confirmed HIGH/CRITICAL findings or undispositioned high-impact investigation items  |
| `release-branch`  | Hard gate — applies the same unresolved-finding rule                                                                |

## Report format

Use three explicit sections:

1. **Confirmed findings**: `File:Line — Severity — Category — Confidence`,
   followed by the exploit scenario, evidence, and recommended fix.
2. **Needs investigation**: file and line, potential impact, category, and
   confidence, followed by the suspected path, uncertainty, and exact evidence
   needed to confirm or exclude it.
3. **Excluded**: the candidate and the documented hard exclusion that proves it
   is not a finding.

Do not lower potential impact merely because confidence is low. Confidence
describes evidence strength; severity describes impact if the path is real.

## Reference files

- [Vuln categories](REFERENCE-vuln-categories.md) — detection guidance per vuln type
- [False positives](REFERENCE-false-positives.md) — hard exclusions + precedent
- [Confidence rubric](REFERENCE-confidence-rubric.md) — scoring methodology (0–10)

## Verify

Confirm the security review report exists, every potentially HIGH or CRITICAL
path appears under `Confirmed findings`, `Needs investigation`, or `Excluded`,
each exclusion cites a hard rule, each detection rule has its positive and
negative fixture pair, and the git context resolves.
