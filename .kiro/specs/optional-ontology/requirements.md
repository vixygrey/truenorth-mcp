# Requirements Document

## Introduction

Optional Ontology makes the ontology feature of the TrueNorth-MCP runtime a per-project
choice. Not every project built with this methodology needs a full domain ontology. Today
the ontology surface is always on: the two ontology tools always register, the
`truenorth://ontology` resource always lists and reads, and the resource auto-creates its
backing file the first time any client reads it. This feature adds a per-project feature
flag that turns the whole ontology surface off, with the default left on so every existing
project keeps its current behavior.

This feature also reconciles a backing-path split that exists in the current source. The
ontology tools read and write `specs/ontology.yaml`, while the ontology resource reads and
creates `.agent/ontology.yml` and treats `specs/ontology.yaml` only as a legacy read
fallback. The `agent-workspace-profiles` design states the ontology backing file is
`.agent/ontology.yml`. This feature moves the tools onto `.agent/ontology.yml` through the
single write guard, keeps a legacy read of `specs/ontology.yaml`, and removes the
ambiguity that would otherwise make a disabled state unclear.

This feature builds on the TrueNorth-MCP refactor
(`.kiro/specs/truenorth-mcp-refactor/`) and Agent Workspace Profiles
(`.kiro/specs/agent-workspace-profiles/`). The write guard (ADR-0008), the `.agent/`
config area (Requirement 1.5 of the profiles spec), and the resource layer all carry
forward. All backward-compatibility guarantees from those specs carry forward.

These requirements cover three areas: the ontology feature flag and its resolution, the
gating of the three ontology surfaces, and the ontology backing-path reconciliation. Design
correctness properties are noted per clause so the design phase can attach P-numbers,
continuing past the profiles spec's P6..P15. This spec follows the strict-tier house
writing rules.

## Glossary

- **ontology feature**: The ontology tools, the ontology resource, and the ontology-scan
  engine, considered as one togglable capability.
- **ontology tools**: The `truenorth_generate_ontology` and `truenorth_verify_ontology`
  MCP tools.
- **ontology resource**: The `truenorth://ontology` MCP resource.
- **ontology feature flag**: The per-project boolean that turns the ontology feature on or
  off. It lives at `features.ontology` in `.agent/config/rules.yml`.
- **feature-default resolution**: The rule the runtime applies when no ontology feature
  flag is declared. The runtime resolves the flag to enabled.
- **ontology backing file**: The single file the ontology tools and the ontology resource
  read and write. Under this spec, that file is `.agent/ontology.yml`.
- **legacy ontology file**: A `specs/ontology.yaml` file from a pre-relocation project. The
  runtime reads it as a fallback but never writes it.
- **empty stub ontology**: The minimal ontology the resource seeds on first read: version
  `1`, an empty domain, no entities, and no constraints.
- **unknown resource**: A resource URI the runtime does not recognize. The runtime returns
  an unknown-resource error for it and does not list it.

## Requirements

### Requirement 1: The Ontology Feature Flag

**User Story:** As a project owner, I want a per-project flag that turns the ontology
feature off, so that a project that does not need a domain ontology is not forced to carry
the ontology tools, resource, and seeded file.

#### Acceptance Criteria

1. THE Runtime SHALL read the ontology feature flag from `features.ontology` in
   `.agent/config/rules.yml`.
2. WHEN the Runtime starts, THE Runtime SHALL resolve the ontology feature flag once and
   SHALL carry the resolved value on the shared server context.
3. IF the `.agent/` directory, the `.agent/config/` directory, or the
   `.agent/config/rules.yml` file is absent, THEN THE Runtime SHALL resolve the ontology
   feature flag to enabled, so that a greenfield project, a bigpowers convert, and an
   existing project with no TrueNorth scaffolding all get an enabled ontology feature.
   (Property: ontology-feature-default-resolution)
4. IF `.agent/config/rules.yml` is present and carries no `features` block, or a `features`
   block with no `ontology` key, THEN THE Runtime SHALL resolve the ontology feature flag to
   enabled. (Property: ontology-feature-default-resolution)
5. IF `features.ontology` is present and set to `true`, THEN THE Runtime SHALL resolve the
   ontology feature flag to enabled.
6. IF `features.ontology` is present and set to `false`, THEN THE Runtime SHALL resolve the
   ontology feature flag to disabled.
7. IF `.agent/config/rules.yml` is present and cannot be read, THEN THE Runtime SHALL
   return an error that names the config path and the failure cause, and SHALL NOT resolve
   a partial flag value.
8. IF `.agent/config/rules.yml` is present and cannot be parsed, THEN THE Runtime SHALL
   return an error that names the config path and the parse cause, and SHALL NOT resolve a
   partial flag value.
9. THE Runtime SHALL preserve every unknown field in `.agent/config/rules.yml` on read, so
   that reading the ontology feature flag does not disturb other config keys.
10. THE Runtime SHALL resolve the ontology feature flag without consulting the `.agent/`
    layout contract, so that a project with no layout contract still resolves the flag.

### Requirement 2: Gate the Ontology Tools

**User Story:** As an agent, I want the ontology tools advertised only when the ontology
feature is enabled, so that I never call a tool the project has turned off.

#### Acceptance Criteria

1. WHEN the ontology feature flag is enabled, THE Runtime SHALL register the
   `truenorth_generate_ontology` and `truenorth_verify_ontology` tools in the tool router.
2. WHEN the ontology feature flag is disabled, THE Runtime SHALL NOT register the
   `truenorth_generate_ontology` or `truenorth_verify_ontology` tools.
   (Property: ontology-disabled-surface-absent)
3. WHEN the ontology feature flag is disabled, THE Runtime SHALL leave every non-ontology
   tool registered and unchanged.

### Requirement 3: Gate the Ontology Resource

**User Story:** As an operator, I want the ontology resource listed, read, and seeded only
when the ontology feature is enabled, so that a disabled project never advertises or writes
an ontology file.

#### Acceptance Criteria

1. WHEN the ontology feature flag is enabled, THE Runtime SHALL list `truenorth://ontology`
   among the served resources.
2. WHEN the ontology feature flag is disabled, THE Runtime SHALL NOT list
   `truenorth://ontology` among the served resources.
   (Property: ontology-disabled-surface-absent)
3. WHEN the ontology feature flag is disabled AND a `resources/read` targets
   `truenorth://ontology`, THE Runtime SHALL treat the URI as an unknown resource and SHALL
   return an unknown-resource error naming the URI.
   (Property: ontology-disabled-surface-absent)
4. WHEN the ontology feature flag is disabled AND a `resources/read` targets
   `truenorth://ontology`, THE Runtime SHALL NOT create the ontology backing file.
   (Property: ontology-disabled-no-seed)
5. WHEN the ontology feature flag is enabled, THE Runtime SHALL keep the current ontology
   resource behavior: list it, read it, and create the empty stub ontology on first read
   when the backing file is absent (profiles spec Requirement 2.4).
6. WHEN the ontology feature flag is disabled, THE Runtime SHALL leave every non-ontology
   resource listed and readable and unchanged.

### Requirement 4: Reconcile the Ontology Backing Path

**User Story:** As a maintainer, I want the ontology tools and the ontology resource to
read and write one backing path, so that the enabled state is consistent and the disabled
state is unambiguous.

#### Acceptance Criteria

1. THE Runtime SHALL resolve the ontology backing file for the ontology tools at
   `.agent/ontology.yml`, matching the ontology resource backing path (profiles spec
   Requirement 2.3). (Property: ontology-single-backing-path)
2. WHEN `truenorth_generate_ontology` writes the ontology, THE Runtime SHALL route the
   write through the single write guard, so the write stays under `.agent/`
   (ADR-0008). (Property: ontology-single-backing-path)
3. WHEN `truenorth_verify_ontology` or `truenorth_generate_ontology` reads the ontology and
   the `.agent/ontology.yml` file is absent, THE Runtime SHALL fall back to reading a legacy
   `specs/ontology.yaml` file when that file is present.
4. WHEN the Runtime writes the ontology, THE Runtime SHALL target `.agent/ontology.yml`
   only, and SHALL NOT mutate a legacy `specs/ontology.yaml` file (profiles spec
   Requirement 1.4). (Property: ontology-single-backing-path)
5. IF `truenorth_generate_ontology` runs AND `.agent/ontology.yml` is absent, THEN THE
   Runtime SHALL seed the ontology and write it.
6. IF `truenorth_generate_ontology` runs AND `.agent/ontology.yml` is present AND the
   existing file is the empty stub ontology, THEN THE Runtime SHALL overwrite the stub with
   the seeded ontology. (Property: ontology-generate-overwrites-stub-only)
7. IF `truenorth_generate_ontology` runs AND `.agent/ontology.yml` is present AND the
   existing file is not the empty stub ontology, THEN THE Runtime SHALL refuse to overwrite,
   leave the file unchanged, and return an error that names `.agent/ontology.yml` and states
   that the human must edit it directly.
   (Property: ontology-generate-overwrites-stub-only)
8. THE Runtime SHALL name `.agent/ontology.yml` in every ontology tool message and every
   ontology resource message that names a backing path, so no message names
   `specs/ontology.yaml` as the write target.
9. THE Runtime SHALL define the empty stub ontology shape in one shared location, and the
   ontology resource seed and the generate-tool stub check SHALL both reference that one
   definition, so the two cannot drift. (Property: ontology-generate-overwrites-stub-only)
10. IF `truenorth_verify_ontology` runs AND the resolved ontology is the empty stub, THEN
    THE Runtime SHALL pass and SHALL return an informational note that the ontology is not
    yet defined, so a pass against an empty ontology does not read as false assurance.
    (Property: ontology-verify-notes-empty-stub)

### Requirement 5: The Ontology Skill Reflects the Flag

**User Story:** As an agent reading the model-domain skill, I want the ontology guidance to
state that the ontology tools apply only when the project enables the ontology feature, so
that I do not try to call absent tools.

#### Acceptance Criteria

1. THE `model-domain` skill SHALL state that the ontology tools apply only when the project
   enables the ontology feature.
2. THE `model-domain` skill SHALL keep its domain-modeling and terminology guidance, which
   does not depend on the ontology feature.
3. THE `model-domain` skill prose SHALL follow the strict-tier house writing rules.

## Out of Scope

- A global or cross-project ontology default. The flag is per project only.
- Tying the ontology feature to a methodology profile. The flag is a standalone per-project
  setting, not a profile field.
- A build-time Cargo feature for the ontology surface. The distributed binary is one build
  and must vary the ontology surface per project at runtime.
- Wiring `truenorth_verify_ontology` into an automatic lifecycle gate. The tool stays an
  explicit call. This spec changes only its availability and backing path.
- New ontology-scan analysis behavior. The regex baseline and the optional tree-sitter AST
  analyzer are unchanged.
- Migrating an existing `specs/ontology.yaml` file into `.agent/ontology.yml`. The runtime
  reads the legacy file but does not move it.
