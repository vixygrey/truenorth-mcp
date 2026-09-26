# ADR-0015: One Mutating Server per Worktree

**Status:** Accepted
**Date:** 2026-09-26

## Context

Runtime mutations read a cockpit file, modify its parsed value, validate the result, and
replace the file atomically. Atomic replacement prevents partial content, but it does not
make the complete read, modify, and write sequence atomic. Concurrent requests can read the
same revision and silently replace each other's updates. Separate MCP clients can also start
separate server processes against the same worktree.

The runtime must preserve its local, file-backed model. A distributed lock, shared database,
multi-tenant service, and concurrent network-filesystem mutation remain out of scope.

## Decision

One TrueNorth server owns mutation rights for one Git worktree.

A server attempts an advisory operating-system lock on `.agent/runtime/writer.lock` at
startup. The first server retains the open file descriptor for its lifetime. A second server
continues to serve safe reads, but each mutating tool returns a typed
`writer_lease_conflict`. It retries the lease on a later mutation, so it can become the
writer after the prior owner exits or crashes. The lock file can remain on disk. The OS lock,
not file presence or recorded process metadata, is authoritative.

One async mutex serializes all mutations inside the owning process. The critical section
starts before the first state-dependent read and ends after the atomic replacement. Resource
notifications happen after the mutex is released.

Each read-modify-write operation records SHA-256 revisions for its destination and any legacy
fallback source. The writer checks those revisions immediately before rename. A mismatch
returns a typed `stale_write_conflict` and leaves the externally edited file unchanged.
Temporary files are created exclusively in the destination directory and renamed atomically.

Parallel agents that need independent mutation use separate Git worktrees. Each worktree has
its own `.agent/` tree and writer lease.

## Consequences

Single-server behavior and successful MCP response shapes stay unchanged. Concurrent calls
inside one server wait rather than lose updates. A second process remains useful for reads,
but it cannot mutate silently.

A process crash releases the advisory lock when the operating system closes the descriptor.
A surviving server can acquire it on its next mutation. The persistent lock file is ignored
by Git and is safe to leave in place.

The lease is local-filesystem coordination for the supported macOS and Linux binaries. It
does not promise correct locking on a shared network filesystem. Digest checking detects an
external edit completed before the commit check. It cannot provide a portable filesystem
compare-and-swap against an uncooperative writer that races the final check and rename.
