---
name: write-document
description: "Create or revise a high-integrity technical document. Use it to write, edit, restructure, or improve an architectural document, technical guide, README, article, or narrative document under specs/."
kind: prose
---

# Write Document

Create or revise technical documentation that is accurate, useful, and durable. Select
the mode from the requested operation and target state:

- **Create mode**: the target document does not exist.
- **Revise mode**: the target document exists.

File existence, current headings, repository conventions, and Git history are facts.
Inspect them directly. Ask the user only for unresolved decisions such as audience,
intended semantic change, policy, or priority.

## Common process

1. Read the request, target document when present, related repository documentation,
   project conventions, and relevant Git history.
2. Determine the mode from the target state and requested operation.
3. Establish the document's purpose, audience, authority, and correct repository
   location.
4. Separate discoverable facts from decisions that require user judgment.
5. Create or revise the document through the applicable process below.
6. Verify claims, structure, links, commands, and stated outcomes against repository
   evidence.
7. Use `simple-english` when deterministic style validation is required.

## Create mode

> **HARD GATE**: Do not create a document unless it provides actionable value to a
> caller, operator, maintainer, or verification path.

### 1. Choose the artifact and location

- **Decision record (ADR)**: a "why" decision under `specs/adr/`.
- **Context map**: system-wide architectural relationships.
- **Technical guide**: operational guidance with verification, commonly a module
  `REFERENCE.md`.
- **Behavioral feature**: an intentional compliance specification.
- **Project README**: project-facing documentation at the repository root.

Place cross-cutting information at the lowest common authoritative location. Link to it
from narrower documents instead of duplicating the content.

### 2. Draft from evidence

- Prefer instructions over descriptions for operational material.
- Link to an ADR, issue, commit, or contract when provenance matters.
- Keep each section at one useful abstraction level.
- Inspect project metadata, commands, configuration, license, and existing guidance
  before declaring information unavailable.
- Use the project README template in [REFERENCE.md](REFERENCE.md) when applicable.

Do not document behavior that does not exist unless the caller is intentionally writing
a specification through the relevant planning workflow.

## Revise mode

> **HARD GATE**: Preserve intent and accuracy. Do not remove, contradict, or materially
> change existing content without understanding why it exists. Read the document,
> relevant repository evidence, and Git history first.

1. Partition the document by its headings and identify each section's purpose.
2. Treat information dependencies as a directed acyclic graph. Order prerequisites
   before the material that depends on them.
3. Improve clarity, coherence, structure, and flow without changing established
   meaning.
4. Preserve identifiers, commands, paths, quoted errors, and external contract text
   unless the requested change explicitly includes them.
5. Resolve stale claims from authoritative repository evidence. Ask only when the
   remaining conflict requires user judgment.

## Shared quality gate

- State boundaries, conditions, and invariants precisely.
- Remove filler, unsupported certainty, and ambiguity without a named condition.
- Explain intent and contracts instead of restating source code.
- Keep one authoritative source and update its indexes or inbound links when needed.
- End operational guidance with an observable next step or verification path.
- Include a verify command only when it proves a meaningful outcome for the document.

Use `audit-code` as a follow-on only when the documentation change exposes or accompanies
implementation risk.
