# ADR-0009: Methodology Profiles as Fixed Data

**Status:** Accepted
**Date:** 2026-09-13

## Context

The bigpowers workflow assumed one methodology: epic-based grouping with a
mandatory epic id. A team that runs kanban, milestones, or a plain issue-per-task
flow could not use the runtime without bending its work into epics.

## Decision

Define five built-in methodology profiles as fixed data: epic-based,
issue-per-task, kanban, milestone-based, and generic. Each profile declares a
grouping vocabulary, whether grouping is required or optional, a starter file set
for `.agent/`, a branch pattern, and whether the commit-msg hook requires an
issue id. The default profile is issue-per-task with optional grouping.

The profiles are a `const` table. The active profile is a single name read from
`.agent/profile.yml` at startup. There is no registration path, so a custom
profile cannot be defined in this spec.

## Consequences

A team picks a profile that matches its workflow, and the runtime, the scaffold,
and the git hooks all read the same declaration. Fixed data keeps the surface
narrow and the behavior fully known. Custom profiles are a future extension.
