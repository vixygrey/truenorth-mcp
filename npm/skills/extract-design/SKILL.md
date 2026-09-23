---
name: extract-design
description: "Extract a DESIGN.md file from an HTML prototype (a design tool export or any styled page) using a headless browser, producing machine-readable tokens and generated prose. Use it when the user has an HTML prototype and wants a DESIGN.md to anchor the project visual identity, or right after a new project is scaffolded."
kind: scripted
verify: node skills/extract-design/tests/test-extraction.js
---

# Extract DESIGN.md from HTML

> **HARD GATE**: Do NOT write DESIGN.md without a headless-browser dual-pass extraction. Tokens from static HTML (a DOM scan, regex, or string scanning) are invalid. They miss the cascade, the custom properties, and the utility-class resolution.
>
> **HARD GATE**: Do NOT claim certainty where the evidence is thin. Flag a low-confidence color role, component classification, or prose assertion with an agent note that states what was observed and asks for validation during grill-me.
>
> **HARD GATE**: Do NOT ship DESIGN.md without running the design-token linter. Unvalidated output is unverified output. The in-repo baseline validator always runs, so this gate holds offline. When the external `@google/design.md` CLI is present, it adds richer checks on top. Flag any lint error in the terminal and in the DESIGN.md prose.

## Flow

1. **Launch the headless browser**: a dual pass, light and dark, with retry and
   timeout. The browser is the sensor. The analysis code is the brain.
2. **Collect the styles**: collect the computed styles from every element. Return
   the raw JSON to the analysis step.
3. **Classify the tokens**: a modular pipeline. Colors (Material 3 roles),
   typography (scale detection), spacing (tolerance GCD), rounding (clustering), and
   components (a visual signature plus pseudo-state variants).
4. **Generate the prose**: heuristics produce all eight DESIGN.md sections. Flag the
   overview and the do's-and-don'ts with an agent note.
5. **Write and validate**: serialize to the project design artifact, run the linter,
   and report to the terminal.
6. **Handoff**: write `handoff.next_skill: grill-me` to `.agent/tasks/state.yml` with the
   uncertain-decisions context.

## Inputs

| Parameter            | Required                         | Description                                                          |
| -------------------- | -------------------------------- | -------------------------------------------------------------------- |
| Source (file or URL) | First run: yes. Update: optional | The HTML prototype path or URL                                       |
| Name                 | No                               | The design-system name, default the page title or the directory name |
| Lint-only            | No                               | Validate an existing DESIGN.md without re-extraction                 |

## Output

- The project design artifact, the canonical design document.
- A terminal summary: the token counts, the component count, the lint result, and
  the uncertain decisions.
- A structured JSON log to stderr: the extraction events, timing, and counts.
- `.agent/tasks/state.yml` `handoff.next_skill: grill-me` with the context.

## Error tiers

| Tier     | Condition                                        | Response                                                       |
| -------- | ------------------------------------------------ | -------------------------------------------------------------- |
| Fatal    | No browser, or a page-load timeout after retries | Exit non-zero, suggest fixes                                   |
| Degraded | Zero colors, zero typography, an SPA shell       | Write DESIGN.md with a degradation warning                     |
| Warned   | A lint error, an uncertain decision              | Write DESIGN.md, flag it in the terminal, hand off to grill-me |

## Dependencies

- A headless browser (a Chrome binary), wrapped behind a browser-extractor
  interface for testability.
- A design-token linter, in two layers. The in-repo baseline validator always runs and
  needs no network. The external `@google/design.md` CLI is an optional enhancement,
  invoked through `npx` at a pinned version. See [REFERENCE.md](REFERENCE.md) for both.

## Verify

Confirm the extraction produced a valid design artifact and the linter passed. The
baseline validator always runs, so a lint result is always present, from the baseline or
the external CLI.

See [REFERENCE.md](REFERENCE.md) for the extraction algorithms and heuristics.
