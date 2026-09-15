# ADR-0012: Optional Ontology Feature Flag

**Status:** Proposed
**Date:** 2026-09-15

## Context

The ontology feature is always on. The two ontology tools always register, the
`truenorth://ontology` resource always lists and reads, and the resource seeds
`.agent/ontology.yml` the first time any client reads it. Not every project built with this
methodology needs a domain ontology, so an always-on ontology surface forces the tools and
a seeded file onto projects that do not want them.

A per-project switch is needed. Two placements were considered. A field on the methodology
profile (ADR-0009) ties the ontology to the grouping methodology, but a project's need for
an ontology is orthogonal to its grouping. A build-time Cargo feature cannot vary per
project, because the distributed binary is one build.

## Decision

Add a per-project ontology feature flag at `features.ontology` in
`.agent/config/rules.yml`. The default is enabled. An absent config file, an absent
`features` block, and an absent `ontology` key all resolve to enabled, so every existing
project keeps its current behavior.

Resolve the flag once at startup with a reader that mirrors `profile::resolve_active`: an
absent file resolves to the default, a present-but-unreadable or unparseable file returns a
typed error naming the config path. The reader deserializes only the `features` block, so
an unrelated `rules.yml` key is tolerated and left untouched. Carry the resolved flag on the
shared server context.

Gate the three ontology surfaces on the flag. When disabled, the ontology router is not
merged, `truenorth://ontology` is not listed, a read of it resolves as an unknown resource,
and no backing file is seeded.

## Consequences

A project turns the ontology feature off with one config line, and a disabled project
advertises no ontology tools or resource and writes no ontology file. The default-enabled
rule keeps every existing project unchanged. The flag reader reuses an existing, required
config file and leaves the methodology profile table focused on workflow shape. The
`features` block leaves room for future per-project toggles without a new file. The
infallible `ServerContext::new` is replaced by a fallible `resolve`, which reads the config
and surfaces a broken file at startup, and an infallible `with_features`, which takes the
flags as data for tests. No constructor hides the fallible read behind an infallible name.
