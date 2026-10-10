# Current task and verification narrative

See [provenance and review status](./README.md) before treating this narrative as accepted authority.
This document describes task ownership. It does not duplicate live execution status.

## Backlog authority

GitHub Issues owns this repository's backlog.
`.agent/tasks/backlog.yml` is the external ownership declaration, not an issue cache.
The current methodology profile determines grouping requirements independently from that declaration.
Record runnable work with `truenorth_record_task`, using neutral grouping and a verify command.
Record externally owned bugs with `truenorth_record_bug`.

## Execution records

`.agent/tasks/release-plan.yml` records task identity, grouping, and verification commands.
`.agent/tasks/execution-status.yml` owns current completion status.
`.agent/tasks/state.yml` owns lifecycle and the active handoff.
Planning specifications, this narrative, and archived checkboxes do not replace those records.

Each task retains the command result and every required observable smoke.
A supporting format or test command cannot certify content review, release approval, or an unperformed workflow action.
Advance lifecycle through `truenorth_advance_phase` rather than invent another phase file.
Use the declared TDD cycle when applicable.

## Compliance group 515

The approved desktop plan maps its implementation steps to GitHub work items #516 through #520.
The [group manifest](../.agent/tasks/compliance-515/group.yml) records the planned boundaries and dependencies.
The live execution record determines which task is active and which has completed.

- #516 establishes typed methodology and skill contracts.
- #517 restores specification provenance, writing policy, and document precedence.
- #518 owns measured quality policy and the real client gate route.
- #519 exercises the intended workflow and retains observable evidence.
- #520 performs the final compliance audit and only separately authorized integration actions.

For #517, the [task ledger](../.agent/tasks/compliance-515/517-tasks.yml) declares its observable acceptance condition.
The retained review and provenance records apply to their recorded 1.0.4 scope.
They do not certify the prepared 1.0.5 authority change.

For #520, the [authority record](../.agent/tasks/compliance-515/520-authority-evidence.json) retains the explicit approval for ADR-0019.
Current verification must exercise the scoped seed paths at the evaluated revision.
It must preserve unresolved historical evidence gaps and must not claim blanket write confinement.
No local step completion claims overall repository compliance or authorizes integration or publication.

## Historical task records

The [archive index](./README.md#recovered-originals) links the three authentic historical task narratives.
Their original checked and unchecked boxes remain unchanged.
Those boxes describe the historical record, not a fresh run against this checkout.
Do not copy them into current execution status or manufacture evidence for missing work.
