# Run Benchmark Reference

## Definition schema version 1

Definitions are JSON. Paths are relative to the definition file. A fixture must resolve
inside that directory and must not contain symbolic links.

```json
{
  "schema_version": 1,
  "skill": "sample-normalizer",
  "runs": 3,
  "timeout_ms": 15000,
  "scenarios": [
    {
      "id": "normalize-label",
      "split": "validation",
      "weight": 1,
      "fixture": "validation/normalize-label",
      "executor": {
        "command": "node",
        "args": ["executor.js"]
      },
      "grader": {
        "command": "node",
        "args": ["grader.js"]
      }
    }
  ]
}
```

Constraints:

- `skill`, scenario ids, and split names are stable report keys.
- `runs` is an integer from 1 through 100.
- `timeout_ms` is optional and ranges from 100 through 120000. The default is 15000.
- A definition must include at least one train and one validation scenario.
- Weights are finite positive numbers.
- Commands are executable names plus argument arrays. Shell command strings are not
  accepted.
- Version 1 supports deterministic code graders only. Model rubrics require a separate,
  explicitly configured adapter and are outside this contract.

## Command environment

The runner passes only `PATH`, `HOME`, temporary-directory names, `SystemRoot` when
present, and these benchmark fields:

| Name                             | Meaning                                            |
| -------------------------------- | -------------------------------------------------- |
| `TRUENORTH_BENCHMARK_MODE`       | `with_skill` or `without_skill`                    |
| `TRUENORTH_BENCHMARK_RUN`        | One-based repetition number                        |
| `TRUENORTH_BENCHMARK_WORKSPACE`  | Fresh disposable fixture root                      |
| `TRUENORTH_BENCHMARK_SKILL_PATH` | Copied `SKILL.md` path, or empty without the skill |
| `TRUENORTH_BENCHMARK_STDOUT`     | Executor stdout file read by the grader            |
| `TRUENORTH_BENCHMARK_STDERR`     | Executor stderr file read by the grader            |

The grader runs only after the executor exits successfully. Both processes use the same
workspace, environment, and timeout. Every mode and repetition gets a new workspace.

## Report schema version 1

The runner writes stable, sorted JSON. It omits timestamps and temporary paths so the same
inputs and outcomes produce identical bytes.

```json
{
  "schema_version": 1,
  "skill": "sample-normalizer",
  "definition_sha256": "<sha256>",
  "skill_sha256": "<sha256>",
  "runs_per_scenario": 3,
  "train": {
    "with_skill": 1,
    "without_skill": 0,
    "delta": 1,
    "scenario_ids": ["train-case"]
  },
  "validation": {
    "with_skill": 1,
    "without_skill": 0,
    "delta": 1,
    "scenario_ids": ["validation-case"]
  },
  "scenarios": [
    {
      "id": "validation-case",
      "split": "validation",
      "weight": 1,
      "fixture_sha256": "<sha256>",
      "with_pass_rate": 1,
      "without_pass_rate": 0,
      "delta": 1,
      "runs": {
        "with_skill": [
          {
            "passed": true,
            "executor": {
              "exit_code": 0,
              "signal": null,
              "timed_out": false,
              "spawn_error": null
            },
            "grader": {
              "exit_code": 0,
              "signal": null,
              "timed_out": false,
              "spawn_error": null
            }
          }
        ],
        "without_skill": []
      }
    }
  ]
}
```

Per-scenario pass rate is `passing runs / runs`. Each split score is
`sum(weight * pass rate) / sum(weight)`. Rates and deltas are rounded to two decimal
places. Validation delta is authoritative for baseline comparison.

## Exit behavior

| Exit | Meaning                                                                                  |
| ---- | ---------------------------------------------------------------------------------------- |
| 0    | Benchmark completed and any baseline check passed                                        |
| 1    | Invalid input, command failure represented in a regressed report, or baseline regression |
| 2    | Required definition, skill, or baseline evidence is unavailable                          |

Individual failed runs remain in a successfully emitted report. They make the process fail
only when `--check-baseline` proves a validation regression. Consumers must inspect run
outcomes and the aggregate delta rather than treating report emission as certification.
