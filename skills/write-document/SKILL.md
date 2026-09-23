---
name: write-document
description: "Write, organize, and sync a high-integrity technical document using the BMAD methodology. Makes every document Bold, Minimal, Actionable, and Durable. Use it to create an architectural doc, a technical guide, or to organize the narrative docs under the specs directory."
kind: prose
---

# Write Document (BMAD)

Create high-signal technical documentation that serves as an expert collaborator for
both a human and an agent. This skill enforces the BMAD principles to prevent context
rot and keep the architecture durable.

Distinct from `edit-document`. Use this skill to create a document that does not yet
exist. Use `edit-document` when a document exists and needs restructuring, clarity,
or prose improvement.

> **HARD GATE**: every document MUST have a clear reason for existence. When a document does not provide actionable leverage for a caller or a test, do not create it.

## The BMAD principles

| Principle      | Execution                                                                                                                                                                             |
| :------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **B**old       | Make a strong assertion. Define a clear boundary and a "never" rule. No "it might" or "usually".                                                                                      |
| **M**inimal    | High density, low filler. When the file exceeds 300 lines or the session exceeds 20 turns, run `terse-mode` and compact the state before saving.                                      |
| **A**ctionable | Link every doc to a verifiable outcome. For an architectural doc, verify through a behavioral feature or a grep-based structure check that proves the design constraints are present. |
| **D**urable    | Design for the long term. Use nested indexing: a root file links to a module-level index, it does not list the individual sub-files.                                                  |

## Process

### 1. Identify the artifact type and scope

Choose the correct artifact.

- **Decision record (ADR)**: for a "why" decision, saved to `specs/adr/`.
- **Context map**: for system-wide architectural mapping.
- **Technical guide**: for a "how-to" with verification, saved to a module
  `REFERENCE.md`.
- **Behavioral feature**: a Gherkin-style compliance spec.
- **Project README**: project-facing documentation, saved to `README.md` at the
  project root.

When a doc affects multiple modules, place the authoritative source in the lowest
common ancestor directory, and use a one-line pointer in each sub-directory to
maintain a single source of truth.

### 2. Draft with semantic velocity

Write the document for expert collaboration.

- Instructions over descriptions: tell the reader exactly how to interact with the
  system.
- Provenance links: link to an ADR, an issue, or a commit to preserve the intent.
- The stepdown rule: information descends exactly one level of abstraction. When a
  root doc needs a leaf-level detail, it points to a sub-index first.

### Quick README (a project README only)

1. Ask for the project name and a one-sentence description.
2. Generate `README.md` at the project root using the template in
   [REFERENCE.md](REFERENCE.md).
3. Fill gaps from the project agent guide when available. Use `TODO` markers
   otherwise.
4. Output, then suggest `edit-document` for polish.

### 3. Apply the quality gate

Before finalizing, audit against these red flags.

- [ ] Filler language: a pleasantry or "I hope this helps". Delete it.
- [ ] Ambiguity: "usually", "often", or "it depends" without a specific condition.
- [ ] Dead end: the document ends with no next step or verification.
- [ ] Shallow content: it restates the code without explaining the intent or the
      contracts.

### 4. Organize

- Place the document in the correct tier: global, then project, then sub-directory.
  A project README is the exception. It goes to the project root.
- Nested indexing: when adding a module-level doc, update the module index doc, and
  add a new module index to the root index.

## Rules

- Minimalism is a requirement. When a document can be a 5-line table, do not make it
  a 5-line essay.
- Verifiable outcomes. Every technical document includes at least one verify command.
- No speculative doc. Do not document a feature that does not exist yet, unless you
  are doing `elaborate-spec`.

Suggest the next skill: `audit-code`.
