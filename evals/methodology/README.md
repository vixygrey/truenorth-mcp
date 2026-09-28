# Methodology evaluations

This suite separates deterministic runtime contracts from agent-mediated methodology behavior.
Code graders are authoritative in both modes. A model score cannot override a failed command,
gate, artifact assertion, or confinement check.

## Deterministic mode

Run the CI-gating subset:

```bash
node scripts/run-methodology-evals.js --mode deterministic --check-baseline
```

Update the baseline only after reviewing scenario, fixture, driver, grader, and assertion changes:

```bash
node scripts/run-methodology-evals.js --mode deterministic --update-baseline
```

The update command refuses failed or skipped scenarios. The baseline records stable assertion
results and test-definition digests. Runtime, bundle, machine, timing, process, temporary-path,
and commit metadata remain report-only.

## Model mode

Model mode is optional and non-gating:

```bash
node scripts/run-methodology-evals.js --mode model
```

Configure a provider-neutral command with:

- `TRUENORTH_EVAL_MODEL_COMMAND`
- `TRUENORTH_EVAL_MODEL_PROVIDER`
- `TRUENORTH_EVAL_MODEL_ID`

The command runs in the disposable workspace and receives a JSON prompt packet on standard input.
It must exit zero after the configured client completes. Without all three settings, every model
scenario reports `SKIP` with an explicit reason. A configured run records provider, model, client
actions, and objective code-grader results.

## Scenario contract

Each scenario owns fixed inputs in `fixture/`, a `scenario.json`, a driver, and a read-only grader.
Drivers may invoke only the bounded context operations supplied by the runner. Declared fixture
mutations represent external events such as implementing a fix. They are not graded as proof that
a prose skill performed the change. Requirement-owner routing and other prose-skill behavior stay
in model mode.

All work occurs under disposable temporary directories. The runner builds the runtime before it
snapshots the source checkout, waits for every child process to exit, checks that graders did not
mutate their scenario, and then removes the workspace.
