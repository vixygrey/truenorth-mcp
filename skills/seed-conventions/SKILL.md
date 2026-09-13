---
name: seed-conventions
description: 'Generate the project agent guide and conventions for a brand-new project through a brief interview, and create the specs/ directory structure. The entry point for a greenfield project. Use it when starting a new project from scratch, or when there is no agent guide yet.'
---

# Seed Conventions

> **HARD GATE**: before any new code lands, confirm the project conventions are understood. Ask: "what does a good commit message look like in this project?".

Bootstrap a new project with the agent conventions it needs. Run it once at the
start of a greenfield project.

## What this creates

- `AGENTS.md`: the canonical, harness-neutral agent guide. This is the single source
  of truth.
- `CONVENTIONS.md`: the shared rules for every agent.
- `specs/`: the directory where the planning output lives.
- Optional per-harness aliases: a symlink or copy of `AGENTS.md` under a
  harness-specific name (for a harness that reads its own file), created only when
  the user opts in. The default output is `AGENTS.md` only.

## Interview

Ask the user these questions, one at a time, waiting for each answer.

1. **Project name and one-sentence description.**
2. **Stack**: the language, framework, and runtime.
3. **Commands**: run, test, build, lint.
   3b. **Preflight** (optional): the command that runs test, lint, and build together.
   When none, chain the three into one preflight row.
4. **Architecture**: the key modules and their relationships, in one or two
   sentences.
5. **Conventions**: any naming, file-organization, or pattern rule every agent must
   follow.
6. **Never-do list**: the hard stops, the things an agent must never touch.
7. **Defensive-code categories**: which apply (rate limit, retry, circuit breaker,
   timeout, graceful degradation).
8. **Per-harness aliases** (optional): whether to also create a harness-specific
   alias of `AGENTS.md`. When no, the standard output is unchanged.

## Writing the generated prose

When writing an instructional line in `AGENTS.md` or `CONVENTIONS.md`, follow the
house writing rules: a directive vocabulary (MUST, MUST NOT, NEVER, ALWAYS, DO, DO
NOT), no hedge modal, at most 20 words per instruction, and one imperative
instruction per line.

## Generate the files

After the interview, generate each file.

- `AGENTS.md`: the canonical agent guide, from the interview answers.
- A per-harness alias: a symlink to `AGENTS.md` (a copy fallback where a symlink
  fails), only when opted in.
- `CONVENTIONS.md`: the standard conventions template plus the project's
  defensive-code categories.

### The specs/ directory

```bash
mkdir -p specs/product specs/product/snapshots specs/epics/archive
mkdir -p specs/tech-architecture specs/adr specs/verifications specs/bugs
touch specs/product/SCOPE_LATEST.yaml specs/product/VISION_LATEST.yaml specs/product/GLOSSARY_LATEST.yaml
touch specs/release-plan.yaml specs/execution-status.yaml specs/planning-status.yaml specs/state.yaml
touch specs/tech-architecture/tech-stack.md
touch specs/bugs/registry.yaml
```

`specs/state.yaml` carries a top-level `workflow_mode` (`team-pr` or `solo-git`,
default `solo-git`). This is the canonical integrate-mode signal for every skill.
Set it once here.

### Self-installing fenced markers

A skill that writes into the agent guide MUST use fenced HTML comment markers, so
handwritten content outside the fence is never clobbered.

```markdown
<!-- BEGIN truenorth:section-id -->

…agent-managed content only…

<!-- END truenorth:section-id -->
```

Merge rule: on update, replace only the content between a matching BEGIN and END
pair. When a marker pair is missing, append a new fenced block at the file end.
Never rewrite the whole file.

Standard marker ids: `project` (seed-conventions), `context-routing`
(seed-conventions), `learned-preferences` (session-state), `tooling`
(setup-environment, guard-git). User prose outside a fence is sacred.

## Checklist

- [ ] `AGENTS.md` exists and is populated.
- [ ] `CONVENTIONS.md` exists and includes the `specs/` output convention.
- [ ] `specs/product/` exists with the scope, vision, and glossary files.
- [ ] `specs/tech-architecture/` exists with the tech-stack doc.
- [ ] `specs/verifications/` and `specs/epics/archive/` exist.
- [ ] `specs/bugs/registry.yaml` exists.
- [ ] Confirm with the user: "does the agent guide accurately describe your project?".
