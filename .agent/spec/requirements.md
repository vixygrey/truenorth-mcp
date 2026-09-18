# Requirements

This file is the machine-facing entry point for the feature narrative. The authored
requirements, design, and tasks live under `.kiro/specs/`. The runtime reads this file as
part of the `.agent/` layout contract.

## Active specs

The project is built from three specs. Each spec holds a `requirements.md`, a `design.md`,
and a `tasks.md`.

- **TrueNorth-MCP refactor**: `.kiro/specs/truenorth-mcp-refactor/`. The passive
  TypeScript catalog server becomes an active, protocol-first Rust MCP runtime. Correctness
  properties P1 to P5.
- **Agent Workspace Profiles**: `.kiro/specs/agent-workspace-profiles/`. The repository
  splits into a human-facing `specs/` layer and a machine-facing `.agent/` layer behind one
  write guard. Five methodology profiles as data. Correctness properties P6 to P15.
- **Optional Ontology**: `.kiro/specs/optional-ontology/`. A per-project `features.ontology`
  flag gates the ontology surface. Correctness properties P16 to P21.

## Product scope

The product scope, vision, and glossary live under `.agent/product/`:

- `.agent/product/scope.yml`: the core value, in-scope and out-of-scope items, constraints,
  and success criteria.
- `.agent/product/vision.yml`: the product vision.
- `.agent/product/glossary.yml`: the domain vocabulary.

## Architecture decisions

The Architecture Decision Records live under `specs/adr/` and are served read-only through
the `truenorth://adr` resource. ADR-0008 through ADR-0013 cover the `.agent/` split, the
methodology profiles, the tracker-owned bug model, the cockpit relocation, and the optional
ontology feature flag.
