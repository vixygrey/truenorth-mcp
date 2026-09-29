# Evolve Skill ADR snippet

```markdown
## ADR-XXXX: Evolve <skill-name>

**Status:** Accepted
**Definition:** path/to/benchmark.json (`<sha256>`)
**Baseline:** path/to/baseline.json, validation delta X
**Result:** path/to/report.json, validation delta Y
**Skill:** `skills/<skill-name>/SKILL.md` (`<sha256>`)
**Change:** one-sentence summary
```

Both reports must use `run-benchmark` report schema version 1 and the same
definition. Keep the reviewed definition outside held-out fixture mutation during
the evolution loop. A missing definition, skill, or baseline is unavailable
evidence and blocks the ADR.
