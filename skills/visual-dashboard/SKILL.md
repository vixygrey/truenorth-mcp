---
name: visual-dashboard
description: 'Start a browser-based dashboard that visualizes the architecture, the implementation plans, and the project status. Reads the cockpit files (state, release plan, epics, planning status) and serves a read-only view.'
---

# Visual Dashboard

> **HARD GATE**: the dashboard is read-only. Do NOT use a visualization to make a decision without consulting the source data. "The chart looks better" is not a decision.

A browser-based visual companion. It visualizes the architecture, the plans, and the
status from the cockpit.

## HTTP cockpit

Start a local server that reads the cockpit and serves it over HTTP.

| Route                                | Purpose                                                                       |
| ------------------------------------ | ----------------------------------------------------------------------------- |
| `GET /api/status?projectDir=<abs>`   | JSON: the state, the release, the epics, the planning status, the active epic |
| `GET /cockpit.html?projectDir=<abs>` | A read-only PM view, planning on the left, epics on the right                 |

The server reads the cockpit files directly. Prefer reading them through the
`truenorth://state` and `truenorth://cockpit` resources when driving the view from
the MCP server.

## Cockpit keys the view reads

- `state.yaml`: `active_flow`, `active_epic_id`, `git`, `handoff`, `epic_cycle`.
- `release-plan.yaml`: `release.version`, and the `epics[]` with `id`, `title`,
  `wsjf`, `file`.
- `execution-status.yaml`: the story and epic status map.
- `planning-status.yaml`: the discover workflows and their status.

## Verify

Confirm the status endpoint returns the current cockpit state for the project.
