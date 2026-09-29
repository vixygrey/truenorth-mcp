---
name: run-benchmark
description: "Run a deterministic skill benchmark from a versioned definition. Isolate with-skill and without-skill contexts, report train and validation deltas, and compare one JSON report schema with a baseline. Use it to measure a skill change against reviewed scenarios."
kind: scripted
verify: node skills/run-benchmark/scripts/tests/run.js
---

# Run Benchmark

> **HARD GATE**: benchmark scores measure change against a baseline. They do not
> certify a skill as correct. A failed or timed-out code grader remains a failure.

Run a versioned benchmark definition with:

```bash
node skills/run-benchmark/scripts/run.js \
  --definition path/to/benchmark.json \
  --skill skills/<name>/SKILL.md \
  --output .agent/tasks/<capsule>/benchmark-<name>.json
```

Use `--update-baseline <path>` to write the current report as the baseline. Use
`--check-baseline <path>` to reject a validation delta below that baseline. These
options are mutually exclusive and consume the same report schema.

## Execution contract

1. Each scenario declares `train` or `validation`, a positive weight, a fixture
   directory, an executor command, and a code-grader command.
2. Every repetition and mode receives a fresh temporary copy of the fixture.
3. The `with_skill` workspace receives the selected skill at
   `.benchmark-context/SKILL.md`. The `without_skill` workspace does not.
4. Executor and grader commands run directly, without a shell, under the same
   bounded environment policy and timeout.
5. Executor stdout and stderr are written inside the disposable workspace. The
   grader receives their paths through documented environment variables.
6. An executor or grader failure, timeout, signal, or spawn error makes that run
   fail. No aggregate score overrides the run result.
7. The runner calculates weighted pass rates and
   `delta = with_skill - without_skill` separately for train and validation.

The report is stable JSON with schema version 1. It includes definition, skill, and
fixture digests; each run outcome; per-scenario pass rates; and weighted train and
validation aggregates. See [REFERENCE.md](REFERENCE.md) for both schemas and the
environment contract.

## Missing evidence

A missing definition, skill file, or baseline emits
`TRUENORTH_BENCHMARK_UNAVAILABLE`, exits 2, and produces no passing evidence.
Malformed definitions, unsafe paths, regressions, and command failures exit 1.
Never invent a score or convert either result into a skip.

Repository methodology evaluations and runtime performance benchmarks are separate
systems. Do not substitute either for a skill benchmark.

## Verify

```bash
node skills/run-benchmark/scripts/tests/run.js
```
