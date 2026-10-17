# Craft Skill — Reference

## Naming rules

Every skill name must be a two-word verb-noun pair:

- First word: a verb, such as survey, model, define, develop, or audit.
- Second word: a noun from the project domain.
- Use kebab-case. Keep the name pronounceable and searchable.

Good: `survey-context`, `audit-code`, `validate-fix`
Bad: `context-surveyor`, `code-auditing-skill`, `fix-validator`

When a justified name cannot follow this pattern, add a concise nonempty
`name_exception` to its frontmatter.

## Skill Structure

```
skill-name/
├── SKILL.md           # Main instructions (required)
├── contract.yml       # Declarative effects, risks, and handoffs (required)
├── REFERENCE.md       # Detailed docs (if needed)
├── EXAMPLES.md        # Usage examples (if needed)
└── scripts/           # Utility scripts (if needed)
    └── helper.sh
```

## SKILL.md templates

Use this template for a prose skill:

```md
---
name: skill-name
description: Brief capability. Use it when specific triggers apply.
kind: prose
---

# Skill Name

## Process

[Concise instructions]
```

Use this template for a scripted skill:

```md
---
name: skill-name
description: Brief capability. Use it when specific triggers apply.
kind: scripted
verify: command that exits nonzero on failure
---

# Skill Name

## Process

[Instructions for the bundled executable content]
```

Add `name_exception: Concise reason.` only when the name does not follow the
verb-noun rule. Do not add `model` or `effort`.

## Contract requirements

`contract.yml` records required metadata for every shipped skill. Keep process detail in
`SKILL.md`.

```yaml
schema_version: 1
name: skill-name
description: Brief capability statement.
scale: task
side_effects: false
risks: false
effect_categories: []
risk_categories: []
preconditions: []
handoffs: []
outputs:
  - result
```

Use one of `atomic`, `task`, `workflow`, or `system` for `scale`. Set
`side_effects` when `effect_categories` is nonempty. Set `risks` when
`risk_categories` is nonempty. Every required handoff must name an existing skill.
Use a kebab-case label for `handoffs[].when`. Declare observable outputs only.

Use only these `effect_categories`: `filesystem_read`, `filesystem_write`,
`network`, `process_execution`, `git_read`, `git_write`, `github_read`,
`github_write`, `external_service_write`, `package_publish`, `deployment`, and
`browser_automation`.

Use only these `risk_categories`: `destructive_operation`, `irreversible_merge`,
`release_publication`, `branch_deletion`, `production_change`, `credential_access`,
and `data_loss`.

## Description Requirements

The description is **the only thing your agent sees** when deciding which skill to load.

**Format**:

- Max 1024 chars
- Write in third person
- First sentence: what it does
- Second sentence: "Use when [specific triggers]"

**Good example**:

```
Investigate a bug by exploring the codebase to find root cause, then write a TDD-based fix plan. The external tracker owns bug detail; cockpit bug references live at .agent/tasks/bugs.json. Use when user reports a bug, wants to investigate a problem, or mentions "triage".
```

## When to Add Scripts

Add utility scripts when:

- Operation is deterministic (validation, formatting)
- Same code would be generated repeatedly
- Errors need explicit handling

## When to split files

The complete `SKILL.md`, including frontmatter, must contain no more than 150 lines.
Move supporting detail into a separate file before the skill reaches that limit.
Split earlier when content has distinct domains or advanced features are rarely
needed.

## Skill registration and validation

After adding a skill directory with SKILL.md, confirm it appears through
`index_skills`. Run `validate_skill` and require a passing structural report. For a
scripted skill, also run the declared frontmatter `verify` command and require exit 0.
