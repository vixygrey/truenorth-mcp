---
name: visual-dashboard
description: "Start a browser-based dashboard that visualizes the architecture, the implementation plans, and the project status. Reads the cockpit files (state, release plan, task groups, planning status) and serves a read-only view."
kind: scripted
verify: node --test skills/visual-dashboard/tests/*.test.cjs
---

# Visual Dashboard

> **HARD GATE**: the dashboard is read-only. Do NOT use a visualization to make a decision without consulting the source data. "The chart looks better" is not a decision.

A browser-based visual companion. It visualizes the architecture, the plans, and the
status from the cockpit.

## HTTP cockpit

Start a local server that reads the cockpit and serves it over HTTP.

| Route                                | Purpose                                                            |
| ------------------------------------ | ------------------------------------------------------------------ |
| `GET /api/status?projectDir=<abs>`   | JSON: profile, state, tasks, optional groups, and execution status |
| `GET /cockpit.html?projectDir=<abs>` | A read-only task and optional group status view                    |

The server reads the cockpit files directly. Prefer reading them through the
`truenorth://state` and `truenorth://cockpit` resources when driving the view from
the MCP server.

## Cockpit keys the view reads

- `.agent/profile.yml`: active methodology profile.
- `.agent/tasks/state.yml`: `active_flow`, `active_task`, optional group, Git, and handoff.
- `.agent/tasks/release-plan.yml`: runtime `tasks[]` and optional neutral `groups[]`.
- `.agent/tasks/execution-status.yml`: task and optional group status maps.
- Legacy epic and story fields are read-only compatibility fallbacks.

## Verify

Run `node --test skills/visual-dashboard/tests/*.test.cjs`. The tests start the real
HTTP server against disposable cockpit data and validate `/api/status`.
