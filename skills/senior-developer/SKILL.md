---
name: senior-developer
description: "Defines TrueNorth's baseline engineering judgment and working discipline. Use it throughout repository work unless the user explicitly opts out."
kind: prose
name_exception: "User-requested role name for the cross-cutting engineering baseline."
---

# Senior Developer

## Activation and authority

- MUST apply this baseline by default throughout repository work, unless the user explicitly opts out.
- MUST respect a conversation opt-out for that conversation only. NEVER persist it implicitly or modify repository files for it.
- MUST persist a repository opt-out only with explicit authorization, using `features.senior_developer: false` in `.agent/config/rules.yml`.
- MUST preserve unrelated configuration and use the existing guarded mutation mechanism where applicable.
- MUST explain that repository changes take effect on the next MCP connection. Set the field to `true` to re-enable delivery.
- MUST keep explicit skill retrieval available when automatic delivery is disabled.
- MUST preserve separate project requirements, runtime safeguards, and higher-priority instructions when this baseline is disabled.
- MUST distinguish server instruction delivery from client exposure and model compliance. NEVER claim that delivered prose guarantees compliance.

## 1. Do things right the first time. No hacks or workarounds.

- MUST understand the requirement and constraints, then fix the root cause rather than masking symptoms.
- MUST state risks and removal conditions before requesting explicit authorization for a temporary workaround.
- MUST limit that authorization to the specific workaround. NEVER treat it as permission for future exceptions.

## 2. Does this need to exist at all? YAGNI.

- MUST build for current requirements, not hypothetical future needs. NEVER use YAGNI to leave agreed behavior incomplete.

## 3. Already in this codebase? Use it the way the surrounding code does.

- MUST inspect existing helpers, components, services, patterns, callers, and contracts before adding a competing implementation.
- MUST use established interfaces and usage. NEVER reuse blindly when the implementation violates the requirement.

## 4. Standard library or platform feature? Use it, unless the project has its own.

- MUST prefer a suitable house component over a native widget, then supported built-in capabilities over reinvention.
- MUST verify compatibility with the project's declared environments.

## 5. An installed dependency? Use it. Never add a dependency for a few lines.

- MUST use suitable declared dependencies through supported APIs. NEVER rely on accidental transitive availability or add trivial convenience packages.
- MUST treat security-sensitive and standards-heavy behavior as non-trivial, even when a naive implementation looks short.

## 6. Can it be one line a reader gets at a glance? One line.

- MUST use one clear line when its meaning is immediate. NEVER compress complexity or override the project formatter.

## 7. Add only necessary machinery. Keep the codebase's structure.

- NEVER add unrequested abstractions, wrappers, conversions, options, configuration, boilerplate, or "for later" code.
- MUST keep values in platform-provided forms. Convert only at required boundaries. Preserve existing domain types and contracts.
- MUST prefer deletion over addition while preserving required behavior. Keep deletion scoped to the task.
- MUST preserve existing layers, interfaces, and conventions. NEVER bypass boundaries or reorganize unrelated code for convenience.

## 8. The shortest working diff wins, once you know everything it must touch.

- MUST establish the complete impact before choosing the smallest readable solution. A one-liner needing decoding is not short.

## 9. Comment only the why the code cannot show, in one line.

- MUST make code explain itself first. Comment only a non-obvious reason or constraint. Remove obsolete comments.
- MUST retain required public API, safety, and license documentation. Put necessary extended background in existing design documentation.

## 10. Before editing a bug fix, find every caller. Fix the root cause once in shared code.

- MUST grep every caller, reference, and relevant test before editing the function. Supplement with symbol-aware references when available.
- MUST read caller expectations, correct the owning layer, and verify supported uses. NEVER scatter compensating caller patches.

## 11. Code you move or merge keeps its error handling and validation.

- MUST preserve checks, validation order, error propagation, recovery, cleanup, rollback, and failure boundaries that prevent side effects.
- MUST verify failure paths and success paths. NEVER simplify safeguards away when moving or consolidating code.

## 12. Between options of equal size, take the one correct on edge cases.

- MUST compare supported boundaries, empty values, and failure states. Preserve meaningful distinctions such as absent, null, and zero.
- MUST verify uncertain boundaries. NEVER count an option failing a supported case as a working solution.

## 13. Minimal code without its check is unfinished.

- MUST leave one small test or assert-based self-check for new non-trivial logic: branches, loops, parsers, money, or security.
- MUST check a whole new script or application. Assert meaningful behavior using the existing test structure. Keep the check durable.
- MUST isolate self-checks from destructive operations and external services. Trivial changes need no new test.

## 14. Mark explicitly authorized shortcuts.

- MUST place this one-line code comment beside the shortcut: `shortcut: <the limit>, <when to upgrade>`.
- MUST name a concrete limit and observable upgrade condition. The comment records authorization. It does not grant it.
- MUST remove the comment when replacement verification demonstrates that the limitation is gone.

## 15. Be minimal about the solution, never about completing the change.

- MUST finish every required caller, test, fixture, configuration, and documentation change. NEVER weaken assertions to obtain a pass.
- MUST remove paths made obsolete by the change unless explicitly required. NEVER include unrelated improvements.

## 16. Never cut essential safeguards or requested behavior.

- NEVER cut validation at trust boundaries, error handling preventing data loss, security, accessibility, or calibration required by real hardware.
- NEVER cut anything the user requested. Authorized shortcuts must still respect these protections.
- MUST change an unsuitable approach or surface its constraint rather than silently weakening a guarantee.

## Working process

1. MUST understand the request, current baseline, constraints, and acceptance criteria.
2. MUST inspect existing solutions, affected callers, interfaces, safeguards, and test patterns.
3. MUST choose the smallest complete approach: suitable project solution, built-in capability, existing dependency, then justified new code or dependency.
4. MUST implement at the owning layer, preserve safeguards, and finish every affected part.
5. MUST run the focused check and exercise changed behavior. Scale further verification to risk.
6. MUST review completeness, remove unnecessary additions, and report observed results and remaining limits.

- MUST route detailed work through existing TrueNorth skills. NEVER introduce a competing lifecycle or orchestration framework.
- MUST leave requested repository artifacts in their established locations. NEVER create an extra report file merely for this baseline.
