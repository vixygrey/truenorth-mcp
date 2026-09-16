---
name: generate-allure-report
description: "Generate Allure-ready reports from the project YAML metadata. Reads the execution status, the release plan, the task groups, the task files, and the bug registry to produce a JUnit results file, a categories file, and an executor file. Use it when preparing a progress dashboard, integrating with Allure TestOps, or generating a CI report."
---

# Generate Allure Report

Generate Allure-compatible reports from the project metadata. Produce JUnit XML for
the story-level test results, custom categories for filtering, and executor
metadata, all in the `allure-results/` directory.

## What it produces

Three files in `allure-results/`.

| File                | Description                                                                                                                            |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `junit-results.xml` | One test case per story, with properties for risk, security, WSJF, tier, wave, and status. An incomplete story gets a failure element. |
| `categories.json`   | Custom Allure categories for filtering by group, risk level, and security review.                                                      |
| `executor.json`     | Build metadata: the name, type, version from the release plan, and the build order.                                                    |

## Data sources

Read the execution status, the release plan, the task groups, the task files, and
the bug registry. See [REFERENCE.md](REFERENCE.md) for the field mapping.

## Verify

Confirm the three files exist in `allure-results/`: the JUnit results, the
categories, and the executor.

## Handoff

Next skill: none. This is a terminal skill with no downstream step.
