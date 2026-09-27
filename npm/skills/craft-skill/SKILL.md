---
name: craft-skill
description: "Create a new skill with proper structure, progressive disclosure, and bundled resources. Use it to create, write, or build a new skill for the lifecycle."
kind: prose
---

# Craft Skill

> **HARD GATE**: Do NOT name a skill without a two-word verb-noun pair. Validate the new skill before you merge it.

## Frontmatter discipline

Every skill declares `name`, `description`, and `kind`. Set `kind` to `prose` or
`scripted`. A scripted skill also declares a nonempty `verify` command. A name that
does not follow the verb-noun rule declares a nonempty `name_exception`.

Do NOT add `model:` or `effort:`. The runtime is model-agnostic and does not route on
a vendor model tier.

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

| Rule            | Limit                                                          |
| --------------- | -------------------------------------------------------------- |
| Sentence length | 20 words or fewer per instruction sentence                     |
| Voice           | Imperative, active                                             |
| Directive terms | MUST, MUST NOT, NEVER, ALWAYS, DO, DO NOT                      |
| Banned modals   | should, might, could, may, consider, try, generally, typically |
| Scope           | The SKILL.md body only, not the `description`                  |

## Process

1. **Gather the requirements**: ask the user what task or domain the skill covers,
   which use cases it must handle, whether it needs executable content or just
   instructions, any reference material to include, and what output it
   produces.
2. **Verify the principles**: the skill is atomic (verb-noun), deep (a simple
   interface over complex internal logic), has hard gates where needed, and is
   verifiable.
3. **Draft the skill**: create the SKILL.md from the matching prose or scripted
   template in [REFERENCE.md](REFERENCE.md). Keep the complete file at 150 lines or
   fewer. Move detail into a reference file before the skill exceeds that cap. When
   the user provides a library README or API docs, extract the triggers and hard
   gates. Do NOT invent an API that is not in the source.
4. **Review with the user**: present the draft, and ask whether it covers the use
   cases, whether anything is missing, and whether any section needs more or less
   detail.
5. **Completion-honesty gate** (HARD GATE): run `validate_skill` on the new skill.
   Fix every failed structural check. For a scripted skill, also run its declared
   `verify` command and record the result. Narration without evidence is rejected.

## Naming rules

Every skill name MUST be a two-word verb-noun pair. A justified exception MUST set a
nonempty `name_exception` in frontmatter. See [REFERENCE.md](REFERENCE.md) for the
full rules and examples.

## The skill output

When the skill produces written output, runtime state goes under `.agent/` and
narrative goes under `specs/`. Document the output-file path in the skill body.

## Review checklist

- [ ] The name is a two-word verb-noun pair or has a nonempty `name_exception`.
- [ ] Frontmatter declares `name`, `description`, and `kind`.
- [ ] `kind` is `prose` or `scripted`.
- [ ] A scripted skill declares a nonempty runnable `verify` command.
- [ ] The description is under 1024 characters, triggers only, no workflow summary.
- [ ] The description includes the "Use it ..." triggers.
- [ ] The complete SKILL.md is 150 lines or fewer.
- [ ] No time-sensitive information.
- [ ] Consistent terminology with the project conventions.
- [ ] The output path is documented when applicable.
- [ ] No repository script references and no vendor model names.

## Verify

Run `validate_skill` for the new skill and require a passing report. For a scripted
skill, also run the declared frontmatter `verify` command and require exit 0.
