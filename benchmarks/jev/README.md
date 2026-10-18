# Jev release validation

Issue #344 remains open until approved provider evidence and published-artifact verification exist.
The checked-in fixtures are synthetic candidate labels. The policy is pending, not a calibrated acceptance baseline.

## Inputs and approval

- `source-v1.yml` records fixture provenance. `fixtures-v1.yml` binds its exact SHA-256 digest.
- `fixtures-v1.yml` labels production rigor and drift cases. Routing and self-heal remain harness-only.
- `policy-v1.yml` records model identity, pricing provenance, required aspects, and acceptance limits.
- `rationales-v1.yml` gives case-by-case reviewer explanations and is not sent to Jev.
- `smoke/` retains five small examples separately from the 100-case calibration candidate.

The calibration candidate contains 50 rigor and 50 drift cases, each with at least
20 positive and 20 negative labels. These template-composed synthetic examples are not
production observations. The separate 100-case holdout under `holdout/` was consumed and
failed; it is historical evidence, **not** fresh validation. Neither corpus is relabeled,
retuned, or reused as the new qualification set.

The operator accepted estimated budgets of USD 0.05 for provider compatibility smoke,
USD 0.25 for calibration, and USD 0.25 for independent validation, with a USD 10.00
provider-side cap. Numerical pilot limits are recorded; overall approval remains pending.

Review labels and numerical limits before changing policy approval to `approved`.
Freeze the policy before independent validation. Do not tune against the validation results.
A model, fixture, source, policy, or scoring change invalidates previous acceptance evidence.
The requested alias does not establish responder identity. Acceptance requires the concrete model recorded in the policy.

## Rigor labeling rubric (rigor-rubric-v1)

This historical evaluator-only rubric defines the four rigor questions as applied to the
calibration and consumed holdout files, **not** to the new proposed-change qualification.
Changing these definitions or question wording invalidates earlier evaluator evidence;
retain the rubric identifier with historical scoring provenance. Label the actual historical
state sent to Jev, not the original file before filtering. Each such rigor state
is an array of `{path, content}` entries for explicitly selected existing files. The protected
reader omits missing files, rejects sensitive paths, drops lines matching the secret denylist
(`secret`, `credentials`, `.env`, or `.pem` patterns), and rejects residual denylist matches.
In this historical evaluator-only mode, proposed write content, dependency resolution
results, repository-wide files, and project rules are **not** supplied automatically.
A fixture can explicitly select a manifest, local module, or convention file as another
evaluated file. Selected context may still be incomplete; missing context alone cannot
establish an import or convention violation. Credential labels
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

Guard confidence and destructive-action gates remain distinct from finding thresholds.
The guard blocks deterministic protected paths and lexical denylist matches without a
provider call; every Jev finding, including high scores, is advisory and cannot hard-block.
The lexical denylist does not purport to detect every credential-shaped literal.

`holdout/fixtures-v1.yml` contains 50 rigor and 50 drift examples, with at least 20
positive and negative examples per signal. It covers moderate versus complex code,
placeholder/redacted inputs versus inert credential material, and justified related-file
changes versus unrelated changes. `holdout/rationales-v1.yml` stays out of provider input.
`holdout/source-v1.yml` freezes the candidate scoring values and rationale digest.
Runtime tests check that this frozen candidate matches default scoring.
The author knew the calibration results: this is a distinct previously held-out dataset,
not blind independent authoring. Its provider attempt failed and consumed it; labels are
preserved as historical evidence, not revised or promoted to fresh qualification.

Keyless preparation can use `JEV_BENCH_FIXTURES=benchmarks/jev/holdout/fixtures-v1.yml`
with the existing offline CLI command. Nested versioned inputs are allowed, but parent
traversal, paths outside `benchmarks/jev/`, and symlink components remain rejected.
Do not change labels or thresholds based on holdout scores. The failed consumed holdout
requires a new candidate and fresh independent validation, not a retry or tuning on it.
No paid repeat calibration or holdout is authorized by this repository change.

Pre-run static label reviews found no blocking invalid labels, but identified coverage limits:
two rigor convention cases rely on function-scoped interpretation of their rule; import cases
do not exercise missing dependency context; drift labels correlate with changed-path count
and explicit task naming, and do not exercise partial scope sprawl or unnamed justified edits.
These coverage limits were present before the failed run and remain relevant historical context.

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

## Proposal-aware guard proof and qualification

The production MCP tool `truenorth_guard_change` and `truenorth-mcp guard` (reading the
proposal on standard input) evaluate `{ "paths": ["src/example.rs"], "content": "fn
area() {}", "task": "Update area" }`.
`content` is **one proposed change**, not the existing contents of each target path.
The guard reads bounded safe disk files for context, reports unavailable context, and
never writes the proposal. New files and missing parent directories need not exist.
Disk-only defects must not be mislabeled as defects in a clean proposal. No result is
a compile-time or credential-validity proof. Deterministic protected-path and lexical
denylist hits block with zero provider calls; model findings and provider failures remain
visible, nonblocking advisories. Feature-off remains the default; changing the `jev`
switch takes effect when MCP reconnects. Guard stdin accepts canonical normalized JSON
with 1-100 paths, at most 1,000,000 bytes of UTF-8 content, and at most 8,000,000 bytes
of total input.
An actual CLI allow/annotate exits zero with empty stdout and advisory stderr; blocks
exit one with stderr. Git `--staged` and `--range` modes only inspect metadata offline:
they are not semantic commit gates. The scaffolded hook consumes this normalized JSON,
including replacement proposals; arbitrary editor/tool envelopes are **not** guessed.
MCP callers can invoke the tool directly. Automatic interception has not been verified
against any real editing client, and is not claimed.

### Guard proposal labeling rubric (guard-proposal-rubric-v1)

Human labels judge the **proposed body** against only the safe context actually available
to the guard. Existing files are evidence, never substitute proposal content.
`hallucinated_import` is positive only where a supplied dependency manifest and local
module context establish an unresolved import; missing/unsupported manifest or omitted
dependency context does not prove an invented import. `violates_conventions` needs an
explicit available project rule contradicted by the proposed code; absent rules do not
establish a violation. `contains_secrets` includes synthetic inert credential-shaped
literals in the proposed code, but excludes obvious placeholders, environment references
and redactions. No real credential is included. The `complexity` Score is **ordinal**:
trivial (straight-line), simple (one loop or branch), moderate (limited interacting
branches), complex (nested interacting state and branches), very complex (multiple
transitions/fallbacks/mutations). `complexity_high` means complex or very complex; it
judges the most complex **proposed function**, not simple helpers or unchanged context.
The current 62.5/100 boundary falls between the moderate and complex rubric levels,
but a model's fractional Score is not a probability or human-level proof.
`drift_out_of_scope` compares changed paths with the supplied task, otherwise cockpit
active task, otherwise only protected paths; the scope request does **not** contain
proposed or disk file bodies, so content-only scope sprawl is not proven. Absent task
context cannot alone prove unrelated work. Model outputs are advisory even when wrong. For
deterministic-only cases model labels are `null`, never inferred negatives. Rationale
text is human-only; provider state never receives it.

`guard-qualification/fixtures-v1.yml` binds 100 **new** proposed-change cases,
50 rigor-focused and 50 scope-focused. All five signals are graded across all 100 cases,
including findings outside a case's focus. Each signal has at least 20 positive and
20 negative labels. Six deterministic-only cases are excluded from model sampling.
Initial files and proposed content are distinct. `source-v1.yml` binds the separate
`rationales-v1.yml` digest; neither
source manifest nor rationales is sent to Jev. Human judgments are synthetic review
candidates, **not** independent provider evidence. The old failed holdout is unchanged.
The code and prompt/context boundaries must freeze before a separately authorized fresh
validation; changing the corpus or policy invalidates its evidence.

Run the new path keylessly with:

```sh
cargo build --locked --manifest-path runtime/Cargo.toml --features jev-http --bin truenorth-mcp
cargo run --locked --manifest-path runtime/Cargo.toml --features jev-http --bin jev-bench -- guard-qualification
node scripts/smoke-mcp-artifact.js --expected-version 1.2.1 -- ./runtime/target/debug/truenorth-mcp
```

The offline fake exercises control flow only; it cannot pass provider acceptance.
The smoke command exercises MCP transport, not live provider compatibility. The
guard-qualification runner calls the **shared production guard**, never independently
reconstructs its final decision. Its guarded aggregate
`.agent/telemetry/jev-guard-qualification.yml` records hashed case IDs, expected/actual
deterministic and final decisions, raw observed finding scores and thresholds,
context-unavailability status, full per-proposal latency, logical calls, HTTP attempts,
token usage/cost provenance, unknown usage and budget interruptions. Failed or incomplete
provider calls are not silently scored negative or free. Independent MCP and stdin
interface regressions provide actual interface evidence; fixture `mcp`/`cli` labels alone
are expectations, not proof those clients ran.

`guard-qualification/policy-v1.yml` is **pending**. Its proposed, separately reviewable
full-guard limits are 50 samples with 20 positive/20 negative per required signal,
agreement at least 98%, no false negatives, at most one false positive, zero successful
path errors, p95 including both calls and retries no more than 10,000 ms, combined
estimated cost per proposal at most USD 0.002, and run estimate at most USD 0.25.
The older evaluator-only limits are not reduced. `JEV_BENCH_CLIENT=http` additionally
requires a positive reviewed `JEV_BENCH_MAX_ESTIMATED_USD`, a separately configured
provider-side cap with `JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED=true`, and under a
pending policy `JEV_BENCH_MEASURE_PENDING=1`. Pending-policy measurements always fail
acceptance; fake responses, incomplete model identity, missing scores, retries with
unobservable usage, and unknown cost cannot pass. The provider-side cap (operator
reported USD 10) is **not** a spend authorization or a verified billing measurement.

The manual protected workflow offers distinct fixed `guard-pilot` and
`guard-qualification` choices, only from the immutable `main` SHA after Environment
approval and separate spending authorization. `guard-pilot` invokes exactly ten reviewed
proposals through the compiled **production MCP server** using the existing session
client. It requires an estimated ceiling at most USD 0.05 and a zero-provider-call
deterministic MCP preflight to establish accounting before any paid proposal. This
integration pilot is not statistical qualification and never accepts production.
`guard-qualification` selects only the new full-guard fixture and policy, with an
estimated ceiling at most USD 0.25. In both modes secrets are injected only into the
prebuilt executable step, no paid run is automatic, and retry/failed-call usage stops
further proposals. Approval here authorizes neither workflow dispatch nor publication.

## Release acceptance

Require approved thresholds, complete aspect coverage, the expected responder, known accounting, and passing independent provider results.
Verify the published native artifact separately. Feature-enabled builds on `main` do not prove an older release contains the HTTP transport.
Keep Jev disabled by default. `features.jev: false` remains the reconnect-effective kill switch.
Provider validation never grants publication authority or bypasses deterministic controls.
