# ADR-0013: Ontology Backing Path Reconciliation

**Status:** Accepted
**Date:** 2026-09-15

## Context

The ontology tools and the ontology resource read and write different backing paths. The
tools target `specs/ontology.yaml` and write through a raw file helper. The resource targets
`.agent/ontology.yml`, treats `specs/ontology.yaml` as a legacy read fallback, and seeds the
`.agent/` file through the single write guard. ADR-0011 relocated the cockpit into `.agent/`
and named `.agent/ontology.yml` as the ontology backing file, but the tools were not
migrated.

The split has two costs. It makes a disabled-ontology state ambiguous, because turning the
feature off must account for two paths. It lets the resource seed and the generate tool
collide: the resource may seed the stub at `.agent/ontology.yml` while the generate tool
writes and overwrite-checks `specs/ontology.yaml`, so the two never see each other.

## Decision

Move the ontology tools onto `.agent/ontology.yml`, matching the resource and ADR-0011.
Route the generate write through the single write guard `write_under_agent`, so the tool
write obeys the same invariant as the resource seed (ADR-0008). Keep a legacy read of
`specs/ontology.yaml` when `.agent/ontology.yml` is absent, mirroring the resource read
path. Never write the legacy file.

Resolve the seed-versus-generate collision with a stub-only overwrite. The generate tool
overwrites `.agent/ontology.yml` only when the existing file is the empty stub the resource
seeds, and refuses to overwrite a real ontology, directing the human to edit it. Correct the
tool and resource messages that name `specs/ontology.yaml` to name `.agent/ontology.yml`.

## Consequences

The ontology has one backing path behind one write guard, so the enabled state is
consistent and the disabled state is unambiguous. A project with an existing
`specs/ontology.yaml` keeps working through the legacy read, and a runtime write never
mutates it. The generate tool succeeds against a resource-seeded stub and still protects a
real ontology from being clobbered. The raw file-write helper in the tools is removed,
because the guard now owns the write. The empty-stub shape is defined once in `engine::spec`
(`Ontology::empty_stub` and `is_empty_stub`), and both the resource seed and the tool check
reference it, so the seed and the overwrite check cannot drift.
