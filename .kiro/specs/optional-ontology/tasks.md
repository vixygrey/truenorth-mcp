# Implementation Plan: Optional Ontology

## Overview

This plan makes the ontology feature a per-project choice and reconciles the ontology
backing path. It adds a feature-flag reader, carries the resolved flag on the server
context, gates the ontology tools and the ontology resource on the flag, and moves the
ontology tools onto `.agent/ontology.yml` behind the single write guard. It then updates
the `model-domain` skill prose.

The runtime crate is written in **Rust** (rmcp, serde, serde_yaml, schemars, tokio). The
crate root is `truenorth-mcp/truenorth-mcp/`. These languages are fixed by the design.

Tasks build incrementally. The feature reader lands and tests before the context field that
carries it. The context field lands before the tool and resource gates that read it. The
path reconciliation is independent of the gate and can land in parallel, but it is sequenced
after the gate so the enabled-parity tests run against the final path. The default-enabled
rule keeps every existing project unchanged, so no task performs a destructive migration.

Each task lands its new definition and that definition's first consumer together, so the
crate stays green under `clippy -D warnings` without the module-scoped `#![allow(dead_code)]`
staging the earlier specs used. The feature reader is consumed by its own tests and the
context field in the same group; the `served_resources` helpers are consumed by the
`server.rs` wiring in the same group.

Property-based and example tests target the design correctness properties, which continue
the profiles design's P6..P15: P16 (feature-default resolution), P17 (disabled surface
absent), P18 (disabled no-seed), P19 (single backing path), P20 (generate overwrites stub
only).

## Tasks

- [x] 1. Feature-flag reader
  - [x] 1.1 Implement `engine::features` (`features.rs`)
    - Define `Features { ontology: bool }` with a `Default` of `ontology: true` (R1.3, R1.4)
    - Define `FeaturesError::{Io, Parse}` naming the config path and cause (R1.7, R1.8)
    - Define `RulesFeatureView` and `FeaturesBlock` that deserialize only the `features`
      block, tolerating and ignoring every other `rules.yml` key (R1.9)
    - Implement `resolve(repo_root)`: read `.agent/config/rules.yml`; absent file returns
      `Features::default` (R1.3); present-unreadable returns `Io` (R1.7); present-unparseable
      returns `Parse` (R1.8); a present file resolves `ontology` from the block, defaulting a
      missing key to `true` (R1.4, R1.5, R1.6)
    - Register the module in `engine::mod`
  - [x] 1.2 Unit tests for `resolve` (P16)
    - Absent `.agent/` resolves enabled; absent `.agent/config/` resolves enabled; absent
      `rules.yml` resolves enabled (the three onboarding shapes: greenfield, bigpowers
      convert, unscaffolded existing project)
    - Present with no `features` block resolves enabled; present with a `features` block but
      no `ontology` key resolves enabled; `ontology: true` resolves enabled; `ontology: false`
      resolves disabled
    - An unrelated `rules.yml` key alongside `features.ontology` still resolves and is left on
      disk; unreadable file returns `Io`; malformed file returns `Parse`
    - The reader resolves without a layout contract present (R1.10)

- [x] 2. Carry the flag on the context
  - [x] 2.1 Add `features: Features` to `ServerContext` (`server.rs`) (R1.2)
    - Replace the infallible `ServerContext::new` with two constructors: a fallible
      `resolve(repo_root) -> Result<Self, FeaturesError>` that calls `features::resolve`, and
      an infallible `with_features(repo_root, Features)` that reads no disk
    - `resolve` is `with_features` composed with `features::resolve`
    - Add `TrueNorthServer::resolve(repo_root) -> Result<Self, FeaturesError>` and a private
      `from_context(ServerContext)` router-assembly helper shared by both constructors
    - Add a `#[cfg(test)]` `TrueNorthServer::test_server(repo_root)` that builds a
      default-enabled server through `with_features`
    - Update `main.rs` to build the server through `TrueNorthServer::resolve`, surfacing a
      broken config as a non-zero exit that names the config path, mirroring
      `config::get_repo_root` handling
    - Migrate the eleven existing construction sites: production (`main.rs`) to `resolve`;
      tests calling `TrueNorthServer::new` to `test_server`; tests building the struct through
      `ServerContext::new` to `with_features(root, Features::default())`
  - [x] 2.2 Tests for the constructors
    - `with_features(root, Features::default())` yields `features.ontology == true`;
      `with_features(root, Features { ontology: false })` yields `false`; `resolve` against a
      disabled `rules.yml` yields `false`; `resolve` against a broken `rules.yml` returns the
      typed error

- [x] 3. Gate the ontology tools (R2)
  - [x] 3.1 Conditional router merge in `from_context` (`server.rs`)
    - Assemble the base router without ontology; add `Self::ontology_router()` only when
      `ctx.features.ontology` is true (R2.1); leave every other router unchanged (R2.3)
  - [x] 3.2 Tests (P17, part 1)
    - Enabled (`test_server`): the router lists both ontology tools; disabled
      (`with_features(root, Features { ontology: false })`): the router lists neither ontology
      tool and still lists a representative non-ontology tool

- [x] 4. Gate the ontology resource (R3)
  - [x] 4.1 Add `served_resources(features)` and `served_from_uri(uri, features)` (`resources/mod.rs`)
    - `served_resources` filters `ALL_RESOURCES`, omitting `Ontology` when the flag is off
      (R3.1, R3.2); `served_from_uri` resolves against the served set (R3.3)
  - [x] 4.2 Wire the two functions into `server.rs`
    - `list_resources` maps `served_resources(self.ctx.features)` (R3.2, R3.6)
    - `read_resource` resolves through `served_from_uri(&request.uri, self.ctx.features)`; a
      `None` maps to the existing unknown-resource error (R3.3); the resolution fails before
      `read_current`, so no seed runs (R3.4)
  - [x] 4.3 Tests (P17 part 2, P18)
    - Disabled: `served_resources` excludes ontology; `served_from_uri` returns `None` for
      the ontology URI; a `read_resource` of the ontology URI returns unknown-resource and
      writes no `.agent/ontology.yml` (assert the file is absent after the read)
    - Enabled: all five resources served; the ontology URI resolves; the seed-on-read
      behavior is unchanged (R3.5)

- [x] 5. Reconcile the ontology backing path (R4)
  - [x] 5.1 Re-point the ontology tool paths (`tools/ontology.rs`)
    - `ontology_path` joins `.agent/ontology.yml` (R4.1); add `legacy_ontology_path` for
      `specs/ontology.yaml` and `ontology_read_path` preferring `.agent/` with the legacy
      fallback (R4.3)
    - `read_ontology` reads through `ontology_read_path`; its not-found message names
      `.agent/ontology.yml` (R4.8)
  - [x] 5.2 One shared empty-stub definition (`engine::spec`) (R4.9)
    - Add `Ontology::empty_stub()` and `Ontology::is_empty_stub(&self)` next to the `Ontology`
      model
    - Re-point `ONTOLOGY_SEED` in `resources/mod.rs` to serialize `Ontology::empty_stub`, so
      the seed and the check share one origin
    - Test: parse `ONTOLOGY_SEED` and assert `is_empty_stub` returns true (pins the two)
  - [x] 5.3 Generate: stub-only overwrite through the write guard
    - When `.agent/ontology.yml` is present and `!existing.is_empty_stub()`, refuse and name
      the path (R4.7); when present and the stub, overwrite (R4.6); when absent, seed (R4.5)
    - Write through `engine::agent_ws::write_under_agent` (R4.2, R4.4); remove the raw
      `write_new_file` helper; the success payload names `.agent/ontology.yml` (R4.8)
  - [x] 5.4 Verify notes the empty stub (`tools/ontology.rs`) (R4.10)
    - Before scanning, when `ontology.is_empty_stub()`, return a pass with a `note` that the
      ontology is not yet defined (R4.10); a filled ontology scans as before
  - [x] 5.5 Correct the stale resource message (`resources/mod.rs`)
    - The ontology parse-error branch names `.agent/ontology.yml` (R4.8)
  - [x] 5.6 Tests (P19, P20, P21)
    - Generate writes `.agent/ontology.yml`; verify reads `.agent/ontology.yml`; verify falls
      back to a legacy `specs/ontology.yaml`; a write leaves the legacy file untouched (P19)
    - Generate against the stub overwrites; generate against a real ontology refuses and
      leaves the file unchanged (P20)
    - Verify against the empty stub passes with the not-yet-defined note; verify against a
      filled ontology scans as before (P21)
    - Update existing ontology tests that assert `specs/ontology.yaml` to assert
      `.agent/ontology.yml` (enabled parity)

- [x] 6. Skill prose (R5)
  - [x] 6.1 Edit `skills/model-domain/SKILL.md`
    - The "Feed the ontology" section states the ontology tools apply only when the project
      enables the ontology feature, and are absent otherwise (R5.1)
    - Keep the domain-modeling and terminology guidance (R5.2); follow the strict-tier house
      writing rules (R5.3)

- [x] 7. Verify the whole feature
  - [x] 7.1 Run `cargo fmt`, `cargo clippy` (deny warnings), and `cargo test`
  - [x] 7.2 Confirm the enabled default: a repo with no `.agent/config/rules.yml` still lists
        the ontology resource and both tools, and seeds `.agent/ontology.yml` on first read
  - [x] 7.3 Confirm the disabled path end to end against a `rules.yml` with
        `features.ontology: false`
