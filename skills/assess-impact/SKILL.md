---
name: assess-impact
description: 'Analyze the blast radius of a proposed change before any code is written. Maps dependents, affected work items, and test coverage. Produces an impact report. Use before plan-work on a non-trivial change, when touching a shared module, or when the user asks "what does this break?".'
kind: prose
---

# Assess Impact

> **HARD GATE** — Run this skill before `plan-work` whenever a change touches an existing module, symbol, or file used by more than one caller. Skip only for net-new code with no existing dependents.

Find the blast radius of the proposed change before a single line is written.

## Modes

- Default: full impact analysis with dependents, affected work items, and test coverage mapping.
- `--lightweight`: fast fan-in/fan-out only. Maps callers and imports without test coverage mapping. Used by execute-group as a pre-plan gate. Risk score above 7 triggers a mandatory grill-me session.

## Process

### 1. Identify the target

Name the symbol, module, or file being changed. If the user hasn't specified, ask one question: "What exactly are you changing?"

### 2. Find dependents

Use language server references (`lsp_find_references`, `lsp_goto_definition`) where available to map symbol callers and implementations accurately across project boundaries.

Fallback when LSP is unavailable:

```bash
# Scope grep to source files, excluding build artifacts and dependencies
git grep -n "[symbol-name]" -- "src/" "lib/"
git log --oneline -10 -- "[file-path]"
```

→ verify: `test -d .agent || test -d skills`

### 3. Map to release-plan work items

Read `.agent/tasks/release-plan.yml` and task-group directories when present. For
each dependent found in Step 2, identify the owning task or work item.

→ verify: `test -f .agent/tasks/release-plan.yml`

### 4. List test coverage

Find tests that exercise the target:

```
grep -rn "[symbol-name]" . --include="*.test.*" --include="*.spec.*"
```

→ verify: `test -d skills`

### 5. Classify risk

| Level  | Condition                                          |
| ------ | -------------------------------------------------- |
| Low    | ≤ 2 callers, all covered by tests                  |
| Medium | 3–10 callers, partial test coverage                |
| High   | > 10 callers, or shared API/interface, or no tests |

### 6. Write the impact report

```
## Target
[symbol or file being changed]

## Dependents ([count])
- [file]: [caller or usage]

## Affected work items
- Task [id]: [title]

## Test Coverage
- [test file]: covers [scenario]
- Gap: [untested behavior]

## Risk: Low / Medium / High
[One-sentence rationale]

## Recommended action
[Proceed / Add tests first / Discuss design]
```

→ verify: the report includes a `Risk:` line.

Suggest `plan-work` once risk is understood and any test gaps are noted.

## Risk score gating

In `--lightweight` mode (used by execute-group), assign a numeric risk score (1–10):

- Fan-in (how many callers): 0–4 points
- Fan-out (how many dependencies the module itself uses): 0–3 points
- Recent churn (git log --oneline -5 count): 0–3 points

**Risk score > 7**: Gate — require a `grill-me` session before proceeding to implementation. Document the grill-me result in the impact report.
