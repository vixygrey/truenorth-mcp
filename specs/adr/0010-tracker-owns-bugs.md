# ADR-0010: The External Tracker Owns Bug Detail

**Status:** Accepted
**Date:** 2026-09-13

## Context

The bigpowers layout stored full bug narratives under `specs/bugs/`, one file per
bug, plus a `.okf.md` sidecar per bug. The repository carried bug detail that a
tracker already owns, and the sidecar format added a second copy of every bug.

## Decision

The external tracker is the source of truth for bug detail. The runtime stores a
lean bug reference in the cockpit file `.agent/tasks/bugs.yml` through the
`truenorth_record_bug` tool. A reference carries an id, an external link, a
status, the linked task or group, and optional tags. The runtime makes no network
request, integrates no tracker API, and stores no tracker credential.

The `specs/bugs/` directory and the `.okf` sidecar format are removed. TrueNorth
does not carry the `.okf` format.

## Consequences

The repository records the link to each bug without duplicating the narrative or
the credentials. A live tracker sync is a future feature, out of scope here.
