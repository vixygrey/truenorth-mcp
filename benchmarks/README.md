# Runtime performance baseline

Informational, non-gating measurements for TrueNorth-MCP. Fresh-process startup does not clear operating-system filesystem caches. Raw samples and full environment metadata are in [runtime-baseline.json](runtime-baseline.json).

## Environment

- Commit: `b1ce728dd6090a35efa61733402b92e3b0aef54e`
- Runtime: `1.0.1`
- Platform: `darwin arm64 25.6.0`
- CPU: Apple M4
- Logical CPUs: 10
- Memory: 25769803776 bytes
- Node: `v24.18.1`
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- Build profile: release

## Timing summary

| Operation                | Median |    Mean |     p95 |     Min |     Max | Unit |
| ------------------------ | -----: | ------: | ------: | ------: | ------: | ---- |
| Fresh-process initialize | 10.471 |  10.309 |  12.664 |   5.946 |  12.763 | ms   |
| Idle resident memory     |  10592 | 10599.2 |   10640 |   10528 |   10640 | KiB  |
| tools/list               |  0.119 |   0.123 |   0.164 |   0.105 |   0.173 | ms   |
| index_skills             |  0.412 |   0.418 |   0.472 |   0.366 |   0.475 | ms   |
| get_skill full           |  0.072 |   0.074 |   0.095 |   0.069 |   0.099 | ms   |
| get_skill reasoning      |  0.079 |   0.079 |   0.094 |   0.073 |   0.099 | ms   |
| get_skill lean           |  0.117 |    0.12 |   0.137 |   0.111 |   0.163 | ms   |
| build_skill_graph        |  2.898 |   2.936 |   3.234 |   2.677 |   3.412 | ms   |
| verify gate round trip   |  2.849 |   2.885 |   3.334 |   2.734 |    3.34 | ms   |
| Watcher response         | 221.47 | 220.983 | 222.453 | 217.947 | 222.492 | ms   |

Estimated verify-gate runtime overhead, excluding the calibrated `/bin/sh -c true` command: **0.028 ms**.

## Skill payloads

Canonical skill: `using-truenorth`. Token estimates use `ceil(Unicode characters / 4)`.

| Tier      | Content bytes | MCP result bytes | Estimated tokens | Ratio to full |
| --------- | ------------: | ---------------: | ---------------: | ------------: |
| full      |          4225 |             4518 |             1057 |             1 |
| reasoning |          4224 |             4516 |             1056 |             1 |
| lean      |          3851 |             4140 |              963 |         0.911 |

## Catalog input

- Skills: 80
- Skill files: 80
- Input bytes: 304381

## Reproduce

```bash
node scripts/benchmark-runtime.js --output benchmarks
```

The suite records raw samples and spread. It defines no regression threshold. Run measurements on an otherwise idle machine and compare only equivalent environments.
