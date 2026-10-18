# Jev release validation

Issue #344 remains open until approved provider evidence and published-artifact verification exist.
The checked-in fixtures are synthetic candidate labels. The policy is pending, not a calibrated acceptance baseline.

## Inputs and approval

- `source-v1.yml` records fixture provenance. `fixtures-v1.yml` binds its exact SHA-256 digest.
- `fixtures-v1.yml` labels production rigor and drift cases. Routing and self-heal remain harness-only.
- `policy-v1.yml` records model identity, pricing provenance, required aspects, and acceptance limits.
- `rationales-v1.yml` gives case-by-case reviewer explanations and is not sent to Jev.
- `smoke/` retains five small examples separately from the 100-case calibration candidate.

The calibration candidate contains 50 rigor and 50 drift cases. Each signal has at least
20 positive and 20 negative examples. Examples are template-composed synthetic cases,
not independent production observations. A distinct holdout review candidate is prepared under
`holdout/`; it has not been evaluated against the provider.
Review the rubric and rationales before authorizing paid calibration.

The operator accepted estimated budgets of USD 0.05 for provider compatibility smoke,
USD 0.25 for calibration, and USD 0.25 for independent validation, with a USD 10.00
provider-side cap. Numerical pilot limits are recorded; overall approval remains pending.

Review labels and numerical limits before changing policy approval to `approved`.
Freeze the policy before independent validation. Do not tune against the validation results.
A model, fixture, source, policy, or scoring change invalidates previous acceptance evidence.
The requested alias does not establish responder identity. Acceptance requires the concrete model recorded in the policy.

## Rigor labeling rubric (rigor-rubric-v1)

This version defines the four production rigor questions and the human labels for calibration.
Changing these definitions or question wording changes the scoring contract and invalidates
earlier acceptance evidence; retain the rubric identifier with aggregate scoring provenance.
Label the **actual state sent to Jev**, not an original file before filtering. Each rigor state
is an array of `{path, content}` entries for explicitly selected existing files. The protected
reader omits missing files, rejects sensitive paths, drops lines matching the secret denylist
(`secret`, `credentials`, `.env`, or `.pem` patterns), and rejects residual denylist matches.
Proposed write content, dependency resolution results, repository-wide files, and project
rules are **not** supplied automatically. A fixture can explicitly select a manifest, local
module, or convention file as another evaluated file. Selected context may still be incomplete;
missing context alone cannot establish an import or convention violation. Credential labels
must likewise reflect only surviving content, never redacted lines.

| Signal                 | Positive label                                                                                              | Negative label                                                                                                                                            |
| ---------------------- | ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hallucinated_import`  | An import is demonstrably unresolved against explicitly supplied project files and dependency declarations. | Imports resolve under supplied context, or that context is insufficient to show an import is invented. A missing manifest or omitted module is not proof. |
| `violates_conventions` | Code contradicts an explicitly supplied project rule or convention.                                         | No supplied rule is contradicted; inferred style preferences and absent rules do not qualify.                                                             |
| `contains_secrets`     | The transmitted code embeds credential material, including inert credential-shaped test values.             | Only obvious placeholders, environment lookups, redactions, or no credential material survive in state. Never use a real key in a fixture.                |

The `complexity` Score has five ordered levels for the supplied code:

| Index | Level        | Definition                                                       |
| ----- | ------------ | ---------------------------------------------------------------- |
| 0     | trivial      | Straight-line code.                                              |
| 1     | simple       | One loop or conditional.                                         |
| 2     | moderate     | Branches or limited nesting with local state.                    |
| 3     | complex      | Deep nesting plus interacting branches or state.                 |
| 4     | very complex | Multiple interacting state transitions, fallbacks, or mutations. |

`complexity_high` is true for complex or very complex, false for trivial, simple, or
moderate. The production Score index is mapped to `index / 4 * 100`; the current configured
candidate boundary of 62.5 is the midpoint between moderate (50) and complex (75),
equivalent to a model Score index of at least 2.5. Because the model Score can be probability-weighted and fractional,
the model threshold is not itself a human level definition. Human fixture labels use the
definitions, not model scores. Calibration and holdout cases remain separate; never tune
against holdout provider results.

## Frozen scoring candidate and holdout

The operator-approved candidate defaults are `rigor_failure_boundary: 0.70` for imports
and conventions, `secret_boundary: 0.25` for credential material, `complexity_boundary: 62.5`,
and `drift_boundary: 0.50`. Projects can override each through the existing `jev` rules block.
The quality acceptance limits are unchanged. These boundaries fit the calibration scores;
they are not independently validated production guarantees.

Guard confidence and destructive-action gates remain separate and unchanged.
A secret score of 0.25 can trigger a rigor annotation without meeting the confidence
required for a guard block. Deterministic path and secret checks are not bypassed.

`holdout/fixtures-v1.yml` contains 50 rigor and 50 drift examples, with at least 20
positive and negative examples per signal. It covers moderate versus complex code,
placeholder/redacted inputs versus inert credential material, and justified related-file
changes versus unrelated changes. `holdout/rationales-v1.yml` stays out of provider input.
`holdout/source-v1.yml` freezes the candidate scoring values and rationale digest.
Runtime tests check that this frozen candidate matches default scoring.
The author knew the calibration results: this is a distinct held-out dataset, not blind
independent authoring. Labels still require independent review before provider evaluation.

Keyless preparation can use `JEV_BENCH_FIXTURES=benchmarks/jev/holdout/fixtures-v1.yml`
with the existing offline CLI command. Nested versioned inputs are allowed, but parent
traversal, paths outside `benchmarks/jev/`, and symlink components remain rejected.
Do not change labels or thresholds based on holdout scores. A failed holdout requires a
new candidate and fresh independent validation rather than tuning against that set.
No paid holdout run or repeat calibration is authorized by preparing these files.

## Keyless verification

```sh
cargo clippy --locked --manifest-path runtime/Cargo.toml --features jev-http --all-targets -- -D warnings
cargo test --locked --manifest-path runtime/Cargo.toml --features jev-http
cargo run --locked --manifest-path runtime/Cargo.toml --features jev-http --bin jev-bench -- bench
```

The last command defaults to the offline fake. Its unseeded evaluations and pending policy fail acceptance with a nonzero exit.
The guarded report at `.agent/telemetry/jev-benchmark.yml` uses schema version **2**. It retains aggregate
measurements and adds `case_diagnostics` for attempted production rigor and drift evaluations only.
Each entry contains a 1-based `case_index` (ordinal in the versioned fixture), `case_id_sha256`
(SHA-256 of the raw fixture `case_id`), the controlled `aspect` (`rigor` or `drift`), and controlled
signal names mapped to `expected`, normalized `score`, normalized `threshold`, and production
`actual` classification. Rigor complexity's score and threshold are divided by 100; the
classification still uses the original 0–100 boundary. Other scores and thresholds are already
on the 0–1 scale. Both boundaries are inclusive; diagnostics do not change the scoring policy.
Use the report's `provenance.fixture_sha256` to select the exact fixture version, then join an
entry to its `cases[case_index - 1]`; verify its `case_id_sha256` by hashing that fixture case's
raw ID. Neither the raw case ID nor fixture content belongs in the report.
Partial/missing answers, failed evaluations, or malformed scores produce an empty `signals` map
and the controlled `evaluation_failed` error, not partial scores or provider error text.
An estimated-budget interruption produces no entry for evaluations it skipped; do not interpret
an absent entry as a negative classification. Deferred routing/self-heal remain aggregate-only.
Earlier schema-1 aggregate reports cannot be retroactively expanded into case diagnostics:
collect new evidence only after separately authorizing any paid provider requests, subject to
the existing spending cap. This diagnostic change requests no paid rerun or holdout run.
Empty, unsupported, or incomplete fixture sets fail before provider calls.

## Protected credential store

Create the GitHub Environment `jev-provider-validation` with required maintainer review and an exact `main` branch restriction.
For a solo maintainer, permit self-review so manual runs can receive approval.
Disable administrator bypass of Environment protection when available.

Add these Environment secrets, not repository-wide secrets:

- `TRUENORTH_JEV_API_KEY`: a dedicated nonproduction key with a provider-side spending cap.
- `TRUENORTH_JEV_ENDPOINT`: the approved HTTPS evaluation endpoint.

The standard endpoint is `https://api.typesafe.ai/v1/systemone`.
Use your provider-approved endpoint for a custom deployment.
Load values through GitHub's Environment secret form or the interactive CLI prompt:

```sh
gh secret set TRUENORTH_JEV_API_KEY --repo vixygrey/truenorth-mcp --env jev-provider-validation
gh secret set TRUENORTH_JEV_ENDPOINT --repo vixygrey/truenorth-mcp --env jev-provider-validation
gh secret list --repo vixygrey/truenorth-mcp --env jev-provider-validation
```

Never put a key in command arguments, repository configuration, MCP registration, logs, or chat.
The list command shows names, not stored values.

## Manual provider smoke

After the workflow reaches `main`, dispatch **Jev provider validation** from `main`.
Enter a reviewed `max_estimated_usd` and confirm the provider-side spending cap before approving the Environment deployment.
The workflow builds and runs deterministic checks without credentials, then injects secrets only into the prebuilt smoke executable.
It runs production rigor and drift compatibility checks and a negative authentication probe.

Leave `provider_control_modes_approved` false unless the provider has approved controlled timeout and retry paths.
Only then add `TRUENORTH_JEV_TIMEOUT_ENDPOINT` and `TRUENORTH_JEV_RETRY_ENDPOINT` as Environment secrets.
Control paths must share the approved endpoint's HTTPS origin. Redirects are disabled.
Do not invent control URLs or induce failures by flooding the provider.
Missing controlled scenarios remain `not_proven`; they cannot produce accepted compatibility evidence.
A pending benchmark policy also prevents acceptance.

The workflow uploads only aggregate evidence tied to source, binary, policy, model, and pricing metadata.
Failed attempts can have unknown billed usage. An estimated successful-response ceiling does not replace a provider-side spending cap.

## Paid calibration

Dispatch **Jev provider validation** from `main` with `mode=calibration`,
`max_estimated_usd=0.25`, and `provider_spend_cap_confirmed=true`.
Leave `provider_control_modes_approved=false`; calibration does not execute compatibility probes.
Approve the protected Environment deployment when GitHub requests it.
The workflow builds the benchmark without secrets and injects credentials only into the
prebuilt calibration executable. It uploads `jev-calibration-aggregate-<source SHA>`.
A pending policy deliberately returns nonzero even after calibration produces measurements;
do not rerun a paid job solely because production acceptance was not granted.

Use the prebuilt benchmark binary in a trusted operator environment with protected endpoint and key injection.
Set `JEV_BENCH_CLIENT=http`, a positive `JEV_BENCH_MAX_ESTIMATED_USD`, and `JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED=true`.
A pending policy additionally requires `JEV_BENCH_CALIBRATE=1`.
Calibration is explicitly nonaccepting until policy limits and labels receive approval.
Never substitute fake-client results for provider evidence.

## Release acceptance

Require approved thresholds, complete aspect coverage, the expected responder, known accounting, and passing independent provider results.
Verify the published native artifact separately. Feature-enabled builds on `main` do not prove an older release contains the HTTP transport.
Keep Jev disabled by default. `features.jev: false` remains the reconnect-effective kill switch.
Provider validation never grants publication authority or bypasses deterministic controls.
