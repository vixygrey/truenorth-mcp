---
name: craft-skill
description: "Create a new skill with proper structure, progressive disclosure, and bundled resources. Use it to create, write, or build a new skill for the lifecycle."
---

# Craft Skill

> **HARD GATE**: Do NOT name a skill without a two-word verb-noun pair. Validate the new skill before you merge it.

## Frontmatter discipline

A skill frontmatter has two fields only: `name` and `description`. Do NOT add a
`model:` field or an `effort:` field. The runtime is model-agnostic. It never routes
on a vendor model tier. Leanness comes from the server render tiers, not the
frontmatter.

## CSO description discipline

The `description` is the catalog selection object, the only field an agent sees when
picking a skill.

| Rule       | Limit                                                                          |
| ---------- | ------------------------------------------------------------------------------ |
| Max length | 1024 characters                                                                |
| Voice      | Third person                                                                   |
| Content    | The capability plus "Use it ..." triggers only                                 |
| Forbidden  | Workflow steps, phase chains, numbered lists, verify commands, HARD GATE prose |

Move the process detail into the SKILL.md body or a REFERENCE.md. Never put it in
the `description`.

## Body discipline

The instructional prose in a skill body MUST follow the house writing rules: short
imperative sentences, active voice, approved modals (`can`, `will`, `must`), and no
vendor model names.

| Rule            | Limit                                                                    |
| --------------- | ------------------------------------------------------------------------ |
| Sentence length | 20 words or fewer per instruction sentence                               |
| Voice           | Imperative, active                                                       |
| Directive terms | MUST, MUST NOT, NEVER, ALWAYS, DO, DO NOT                                |
| Banned modals   | should, might, could, may, consider, try, generally, typically           |
| Scope           | The SKILL.md body only, not the `description`, not the terse-mode output |

## Process

1. **Gather the requirements**: ask the user what task or domain the skill covers,
   which use cases it must handle, whether it needs executable content or just
   instructions, any reference material to include, and what output it
   produces.
2. **Verify the principles**: the skill is atomic (verb-noun), deep (a simple
   interface over complex internal logic), has hard gates where needed, and is
   verifiable.
3. **Draft the skill**: create the SKILL.md with concise instructions (see
   [REFERENCE.md](REFERENCE.md) for the template), plus a reference file when the
   content exceeds 100 lines. When the user provides a library README or API docs,
   extract the triggers and the hard gates. Do NOT invent an API that is not in the
   source.
4. **Review with the user**: present the draft, and ask whether it covers the use
   cases, whether anything is missing, and whether any section needs more or less
   detail.
5. **Completion-honesty gate** (HARD GATE): before you declare the skill done,
   validate that the name is a verb-noun pair, the description is 1024 characters or
   fewer with triggers only, the body follows the writing rules, and the skill
   parses. Show the evidence. Narration without evidence is rejected.

## Naming rules

Every skill name MUST be a two-word verb-noun pair. See [REFERENCE.md](REFERENCE.md)
for the full rules, examples, and the documented exceptions.

## The skill output

When the skill produces written output, runtime state goes under `.agent/` and
narrative goes under `specs/`. Document the output-file path in the skill body.

## Review checklist

- [ ] The name is a two-word verb-noun pair (or a documented exception).
- [ ] The frontmatter is `name` and `description` only.
- [ ] The description is under 1024 characters, triggers only, no workflow summary.
- [ ] The description includes the "Use it ..." triggers.
- [ ] The SKILL.md body is under 100 lines.
- [ ] No time-sensitive information.
- [ ] Consistent terminology with the project conventions.
- [ ] The output path is documented when applicable.
- [ ] No repository script references and no vendor model names.

## Verify

Confirm the new skill parses, its name is a verb-noun pair, its description is
within the limit, and its body follows the writing rules.
