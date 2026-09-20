# Requirements Document

## Introduction

This feature adds an evaluation harness for the TypeSafe Jev "System One" model API.
The harness measures whether Jev is a good fit for five product aspects of
TrueNorth-MCP. The harness is not a live wiring into the request path. No tool, gate,
or resource in the live runtime calls Jev as a result of this feature.

The five aspects under evaluation are tool routing, parallel rigor scoring, drift
guardrails, context pruning, and self-healing decisions. Each aspect gets a harness
module, a set of unit and property tests driven by a named fake client, and a
benchmark case that reports latency, confidence, and agreement metrics.

Jev is a network dependency on an external, US-hosted, closed-weights service. This
cuts against three TrueNorth design principles: disk is the source of truth, the
runtime is offline-friendly, and the runtime is model and harness agnostic. Therefore
the whole capability sits behind an optional feature flag that defaults to off. When
the flag is off, no Jev code path activates and no network call is made. When the flag
is off, `cargo test` still runs the full suite against the fake client, offline and
deterministic.

The Jev API key is a secret. The harness reads the key from the environment. The
harness never writes the key to any `.agent` file, any memory file, or any committed
document. The harness never echoes the key.

## Glossary

- **Jev_Client**: The project-owned trait that abstracts one Jev evaluation request. Two
  types satisfy the trait: the Fake_Client and the Http_Client.
- **Fake_Client**: A named test type that satisfies Jev_Client and returns configured
  responses with no network call.
- **Http_Client**: The type that satisfies Jev_Client by a direct HTTPS call to the Jev
  endpoint. The Http_Client activates only when the Jev_Feature_Flag is on and the API
  key is present.
- **Jev_Feature_Flag**: The per-project flag named `jev` in the `features` block of
  `.agent/config/rules.yml`. The default value is off.
- **Jev_Request**: One request body sent to the Jev endpoint. The body carries `state`,
  `model` set to `jev-latest`, and a `questions` map.
- **Question**: One typed item in the `questions` map. The type is Noul, Choice, or
  Score.
- **Noul**: A yes-or-no Question. The response carries a `noul` value from 0 to 1. A Noul
  carries no confidence value.
- **Choice**: A Question over a fixed option set. The response carries the chosen option,
  a probability map that sums to 1, and a confidence from 0 to 1.
- **Score**: A Question over ordered levels. The response carries a probability-weighted
  score, a legend, a probability map, and a confidence from 0 to 1.
- **Confidence_Band**: The mapping from a confidence value to one of three actions: high
  acts automatically, medium proceeds with confirmation, low escalates to a human.
- **Confidence_Threshold**: The configured cutoff between two Confidence_Band values. A
  destructive action uses a higher threshold.
- **Secret_Denylist**: The load-bearing pattern list in `.agent/config/rules.yml` that
  names file content the runtime never reads, echoes, or sends outward.
- **Protected_Path**: One entry in the `protected_paths` list of
  `.agent/config/rules.yml`. The current list is `specs/`, `specs/adr/`, `LICENSE`, and
  `.github/workflows/`.
- **Rigor_Check**: The parallel evaluation of one code or state input across several
  isolated questions in one Jev_Request.
- **Routing_Target**: One tool the router can pick. The set is the actual TrueNorth tool
  surface, listed in Requirement 3.
- **Benchmark**: The harness runner that exercises the five aspects against real
  repository data and reports latency, confidence, and agreement metrics.
- **Harness**: The full evaluation feature: the Jev_Client trait, the two clients, the
  five aspect modules, the tests, and the Benchmark.

## Requirements

### Requirement 1: Optional Jev feature flag

**User Story:** As a runtime operator, I want the Jev harness behind an optional flag,
so that the runtime stays offline-friendly and free of a hard network dependency.

#### Acceptance Criteria

1. THE Harness SHALL read the Jev_Feature_Flag from the `features` block of `.agent/config/rules.yml`.
2. WHERE the `features` block has no `jev` key, THE Harness SHALL resolve the Jev_Feature_Flag to the value off.
3. WHERE the `.agent/config/rules.yml` file is absent, THE Harness SHALL resolve the Jev_Feature_Flag to the value off.
4. WHILE the Jev_Feature_Flag is off, THE Harness SHALL make no network call to the Jev endpoint.
5. WHILE the Jev_Feature_Flag is off, THE Harness SHALL construct no Http_Client and SHALL run no Http_Client code path.
6. IF the `.agent/config/rules.yml` file is present and cannot be read, THEN THE Harness SHALL return a typed error that names the file path and the read failure cause, and SHALL resolve no partial flag value.
7. IF the `.agent/config/rules.yml` file is present and cannot be parsed, THEN THE Harness SHALL return a typed error that names the file path and the parse failure cause, and SHALL resolve no partial flag value.
8. THE Harness SHALL resolve the Jev_Feature_Flag by the same reader pattern that resolves the `ontology` flag.

### Requirement 2: Jev client trait and two clients

**User Story:** As a runtime developer, I want a project-owned client trait with a fake
and a real client, so that the test suite runs offline and the real call is isolated.

#### Acceptance Criteria

1. THE Harness SHALL define the Jev_Client trait as the single narrow interface for one Jev evaluation.
2. THE Fake_Client SHALL satisfy the Jev_Client trait as a named type.
3. THE Http_Client SHALL satisfy the Jev_Client trait as a named type.
4. WHILE a test runs, THE Harness SHALL use the Fake_Client and SHALL make no network call.
5. WHERE the Jev_Feature_Flag is on and the API key is present, THE Harness SHALL use the Http_Client with a wall-clock timeout of 30 seconds for the Jev call.
6. WHERE the Jev_Feature_Flag is on and the API key is absent, THE Harness SHALL return a typed error that names the missing environment variable and the remediation step, and SHALL make no network call.
7. WHERE the Jev_Feature_Flag is off, THE Harness SHALL not construct the Http_Client and SHALL make no network call.
8. IF the Jev call fails, times out, or returns a non-success response, THEN THE Http_Client SHALL return a typed error that names the failure cause, and SHALL preserve no partial result.
9. THE Http_Client SHALL send the `Authorization` header as a bearer token and the `Content-Type` header as `application/json`.
10. THE Http_Client SHALL set the `model` field of every Jev_Request to `jev-latest`.

### Requirement 3: Tool routing evaluation

**User Story:** As a runtime maintainer, I want to measure Jev routing quality against
the real tool set, so that I can judge a pre-processor that picks the next tool.

#### Acceptance Criteria

1. THE Harness SHALL build one Choice Question whose option set is the Routing_Target set.
2. THE Routing_Target set SHALL be `truenorth_scaffold_project`, `truenorth_advance_phase`, `truenorth_record_task`, `truenorth_verify_gate`, `truenorth_tdd_cycle`, `truenorth_record_bug`, `truenorth_generate_ontology`, `truenorth_verify_ontology`, `index_skills`, `get_skill`, `read_skill`, `search_skills`, `get_dependencies`, `get_git_context`, `validate_skill`, and the decline option `NONE`.
3. WHEN the routing Choice returns a chosen option inside the Routing_Target set, THE Harness SHALL map the option to exactly one member of the Routing_Target set.
4. IF the routing Choice returns an option outside the Routing_Target set, THEN THE Harness SHALL return a typed error that names the unexpected option and the expected option set, and SHALL record no route.
5. WHEN the routing Choice returns a confidence at or more than the high Confidence_Threshold, THE Harness SHALL mark the route as automatic.
6. WHILE the routing confidence is less than the high Confidence_Threshold and at or more than the low Confidence_Threshold, THE Harness SHALL mark the route as needs-confirmation.
7. IF the routing confidence is less than the low Confidence_Threshold, THEN THE Harness SHALL mark the route as escalate-to-human.
8. THE Harness SHALL record the chosen Routing_Target, the confidence, and the Confidence_Band for each routing case.

### Requirement 4: Parallel rigor scoring evaluation

**User Story:** As a runtime maintainer, I want one request to score several rigor
signals at once, so that I can judge parallel isolated evaluation.

#### Acceptance Criteria

1. THE Harness SHALL build one Jev_Request that carries exactly four questions against the same code or state input, one question per rigor signal in criteria 2 to 5.
2. THE Rigor_Check SHALL include a Noul for hallucinated-import probability, with a value from 0.0 to 1.0 inclusive.
3. THE Rigor_Check SHALL include a Noul for violates-project-conventions probability, with a value from 0.0 to 1.0 inclusive.
4. THE Rigor_Check SHALL include a Score for complexity, with an integer value from 0 to 100 inclusive.
5. THE Rigor_Check SHALL include a Noul for contains-secrets probability, with a value from 0.0 to 1.0 inclusive.
6. WHEN the Rigor_Check response arrives, THE Harness SHALL read each of the four answers by its caller-chosen question id.
7. IF a caller-chosen question id is absent from the response, THEN THE Harness SHALL reject the response, retain no partial answer, and return an error indicating the missing question id.
8. IF two answers in the response carry the same caller-chosen question id, THEN THE Harness SHALL reject the response, retain no partial answer, and return an error indicating the duplicate question id.
9. WHEN the Harness reads a valid Rigor_Check response, THE Harness SHALL record the four answers and one latency value in milliseconds for the single Jev_Request.
10. IF a Jev_Request would exceed the 32000-token request budget, THEN THE Harness SHALL reject the Jev_Request before the call, send no request, and return an error indicating the token budget was exceeded.

### Requirement 5: Drift guardrail evaluation

**User Story:** As a runtime maintainer, I want a deterministic in-scope judgment for an
agent plan, so that I can judge Jev as a guardrail against agentic drift.

#### Acceptance Criteria

1. THE Harness SHALL build one Noul that judges whether an agent plan is out of scope.
2. THE drift Noul SHALL reference the actual `protected_paths` list: `specs/`, `specs/adr/`, `LICENSE`, and `.github/workflows/`.
3. THE Harness SHALL read the drift boundary on the `noul` value as a configured value in `.agent/config/rules.yml`, with a range from 0 to 1.
4. WHEN the drift Noul returns a `noul` value at or more than the configured boundary, THE Harness SHALL mark the plan as out-of-scope.
5. WHILE the drift Noul returns a `noul` value less than the configured boundary, THE Harness SHALL mark the plan as in-scope.
6. THE Harness SHALL apply the drift boundary to the `noul` value inside the Harness, not inside the Jev model, and this threshold step SHALL carry the determinism.
7. THE Harness SHALL apply the same boundary value to the same `noul` value and SHALL return the same pass-or-fail result.
8. WHEN an agent plan writes a path that matches a `protected_paths` entry by a literal path match, THE Harness SHALL mark the plan as out-of-scope through a model-free layer that makes no Jev call.
9. THE Harness SHALL record the `noul` value and the pass-or-fail result for each drift case.

### Requirement 6: Context pruning evaluation

**User Story:** As a runtime maintainer, I want per-line relevance scoring for a large
log, so that I can judge Jev as a context-pruning aid.

#### Acceptance Criteria

1. THE Harness SHALL build one Score that rates the relevance of a log line to the task named in the Jev_Request `state`.
2. WHEN the pruning Score returns a score for each line, THE Harness SHALL keep every line at or more than the configured keep threshold.
3. WHEN the pruning Score returns a score for each line, THE Harness SHALL drop every line less than the configured keep threshold.
4. THE Harness SHALL preserve the original order of the kept lines.
5. IF the whole log exceeds the 32000-token request budget, THEN THE Harness SHALL split the log into ordered chunks, and each chunk SHALL stay at or less than the 32000-token request budget.
6. THE Harness SHALL record the kept-line count, the dropped-line count, and the input-line count for each pruning case.
7. THE kept-line count plus the dropped-line count SHALL equal the input-line count.

### Requirement 7: Self-healing decision evaluation

**User Story:** As a runtime maintainer, I want a typed remediation decision on a rigor
failure, so that I can judge Jev for deterministic self-healing.

#### Acceptance Criteria

1. WHEN a Rigor_Check returns any Noul at or more than the configured rigor-failure boundary, or a complexity Score at or more than the configured complexity boundary, THE Harness SHALL build one Choice over the fixed option set `REVERT`, `REFACTOR_IMPORTS`, `SIMPLIFY_LOGIC`, and `ASK_HUMAN`.
2. WHEN the self-healing Choice returns a chosen option inside the fixed set and no confidence override applies, THE Harness SHALL map the option to exactly one typed instruction value.
3. IF the self-healing Choice returns an option outside the fixed set, THEN THE Harness SHALL return a typed error that names the unexpected option and the expected option set, and SHALL return no instruction value.
4. IF the self-healing confidence is less than the low Confidence_Threshold, THEN THE Harness SHALL return the `ASK_HUMAN` instruction and SHALL override the mapped option.
5. IF the chosen option is `REVERT` and the self-healing confidence is less than the destructive-action Confidence_Threshold, THEN THE Harness SHALL return the `ASK_HUMAN` instruction and SHALL return no `REVERT` instruction.
6. THE Harness SHALL record the chosen instruction and the confidence from 0 to 1 for each self-healing case.

### Requirement 8: Confidence banding

**User Story:** As a runtime maintainer, I want a shared three-band confidence policy, so
that every aspect maps confidence to an action the same way.

#### Acceptance Criteria

1. THE Harness SHALL map a Choice confidence value or a Score confidence value to one Confidence_Band: high, medium, or low.
2. WHEN a Choice confidence value or a Score confidence value is at or more than the high Confidence_Threshold, THE Harness SHALL resolve the Confidence_Band to high.
3. WHILE a Choice confidence value or a Score confidence value is less than the high Confidence_Threshold and at or more than the low Confidence_Threshold, THE Harness SHALL resolve the Confidence_Band to medium.
4. IF a Choice confidence value or a Score confidence value is less than the low Confidence_Threshold, THEN THE Harness SHALL resolve the Confidence_Band to low.
5. WHERE a Question is a Noul, THE Harness SHALL apply a configured boundary on the `noul` value rather than a Confidence_Band, because a Noul carries no confidence value.
6. THE Harness SHALL read the high and low Confidence_Threshold values as configurable inputs.
7. IF the low Confidence_Threshold is more than the high Confidence_Threshold, or either threshold is outside the range 0 to 1, THEN THE Harness SHALL return a typed error that names the invalid threshold pair and SHALL resolve no Confidence_Band.
8. WHERE an action is destructive, THE Harness SHALL apply a higher Confidence_Threshold than a non-destructive action.
9. THE Harness SHALL apply the same confidence value and the same thresholds and SHALL resolve the same Confidence_Band.

### Requirement 9: Secret protection

**User Story:** As a security-conscious operator, I want the harness to strip secrets
before any outbound call, so that repository secrets never reach the external service.

#### Acceptance Criteria

1. WHEN the Harness builds the `state` for a Jev_Request, THE Harness SHALL exclude a file whose path matches a Secret_Denylist pattern.
2. WHEN the Harness builds the `state` for a Jev_Request, THE Harness SHALL exclude file content that matches a Secret_Denylist pattern.
3. THE Harness SHALL read the API key from an environment variable.
4. IF the API-key environment variable is absent, THEN THE Harness SHALL return a typed error that names the missing environment variable and the remediation step, and SHALL make no network call.
5. THE Harness SHALL write the API key to no `.agent` file, no memory file, and no committed document.
6. THE Harness SHALL exclude the API key from every log line, every error message, and every benchmark report.
7. IF a Jev_Request `state` still contains content that matches the Secret_Denylist after the exclusion step, THEN THE Harness SHALL return a typed error and SHALL make no network call.

### Requirement 10: Jev API error handling

**User Story:** As a runtime developer, I want typed handling of every Jev error status,
so that a failed call surfaces an actionable message and never panics.

#### Acceptance Criteria

1. IF the Jev endpoint returns status 401, THEN THE Http_Client SHALL return a typed unauthorized error that names the API-key environment variable and the remediation step.
2. IF the Jev endpoint returns status 422, THEN THE Http_Client SHALL return a typed validation error that names the offending field reported by the response body.
3. IF the Jev endpoint returns status 429, THEN THE Http_Client SHALL retry with exponential backoff up to the configured retry limit.
4. IF the Jev endpoint returns status 529, THEN THE Http_Client SHALL retry with exponential backoff up to the configured retry limit.
5. IF the retry limit is reached, THEN THE Http_Client SHALL return a typed error that names the last status and the retry count.
6. THE Http_Client SHALL raise no panic on any error status.
7. THE Http_Client SHALL return a typed error rather than call `unwrap` or `expect` on a failed response.

### Requirement 11: Benchmark and metrics

**User Story:** As a runtime maintainer, I want a benchmark over real repository data, so
that I can compare the five aspects on latency, confidence, and agreement.

#### Acceptance Criteria

1. THE Benchmark SHALL exercise the five aspects against real repository data.
2. THE Benchmark SHALL run against either the Fake_Client or the Http_Client, selected by configuration.
3. THE Benchmark SHALL report a latency metric in milliseconds for each aspect, measured per invocation.
4. WHERE an aspect returns a confidence, THE Benchmark SHALL report the confidence metric for that aspect as a value from 0.0 to 1.0.
5. THE Benchmark SHALL report an agreement metric for each aspect, computed as the fraction from 0.0 to 1.0 of fixture cases where the Jev answer matches the expected answer.
6. THE Benchmark SHALL write the report under `.agent/telemetry/`, and SHALL exclude the API key and any Secret_Denylist content from the report.
7. WHERE the Benchmark runs against the Fake_Client, THE Benchmark SHALL make no network call.
8. THE Benchmark SHALL load a labeled fixture set that pairs each real repository case with an expected answer for each aspect.
9. IF the labeled fixture set is missing or cannot be parsed, THEN THE Benchmark SHALL stop before it runs any aspect, retain any prior report, and return an error indicating the missing or unparsable fixture set.
10. THE Benchmark SHALL run as a separate runner that is not part of `cargo test`.

### Requirement 12: Cost accounting with unverified price

**User Story:** As a runtime maintainer, I want cost reported from a configurable price,
so that the harness never hardcodes an unconfirmed Jev price.

#### Acceptance Criteria

1. THE Harness SHALL account for the input token count and the output token count as two separate values.
2. WHERE a configured price reports output tokens as free, THE Harness SHALL apply a zero price to the output token count.
3. THE Harness SHALL read the per-token price as a configurable input.
4. THE Harness SHALL treat the per-token price as unverified.
5. IF a configured per-token price is negative or non-numeric, THEN THE Harness SHALL return a typed error that names the invalid price value and SHALL report no monetary cost.
6. WHERE no per-token price is configured, THE Harness SHALL report the input token count and the output token count and SHALL report no monetary cost.
7. WHEN a per-token price is configured, THE Harness SHALL report an estimated cost and SHALL label the estimate as based on an unverified price.
8. THE Harness SHALL hardcode no per-token price in any acceptance-tested code path.

## Correctness Properties

These properties continue the repository property numbering from P22. Each property
suits a property-based test with the `proptest` crate. The Fake_Client drives every
property test with no network call.

- **P22 (Flag-off silence).** For all repository states, while the Jev_Feature_Flag is
  off, the Harness makes no network call and activates no Http_Client path (Requirement
  1.4, 1.5).
- **P23 (Confidence-band totality).** For all confidence values from 0 to 1 and all
  valid threshold pairs, the Harness resolves exactly one Confidence_Band (Requirement
  8.1 to 8.4).
- **P24 (Confidence-band determinism).** For all confidence values and all threshold
  pairs, two resolutions of the same input return the same Confidence_Band (Requirement
  8.9).
- **P25 (Drift boundary determinism).** For all `noul` values and all boundary values,
  two evaluations of the same input return the same pass-or-fail result (Requirement
  5.7).
- **P26 (Pruning line conservation).** For all input logs and all keep thresholds, the
  kept-line count plus the dropped-line count equals the input-line count (Requirement
  6.7).
- **P27 (Pruning order preservation).** For all input logs and all keep thresholds, the
  kept lines hold their original relative order (Requirement 6.4).
- **P28 (Routing option closure).** For all routing Choice responses, the mapped
  Routing_Target is a member of the Routing_Target set (Requirement 3.2, 3.3).
- **P29 (Self-healing option closure).** For all self-healing Choice responses inside the
  fixed option set, the Harness maps the option to a typed instruction, and for any
  option outside the set, the Harness returns a typed error (Requirement 7.2, 7.3).
- **P30 (Low-confidence self-healing safety).** For all self-healing responses with a
  `REVERT` option and a confidence less than the destructive-action threshold, the
  Harness returns the `ASK_HUMAN` instruction (Requirement 7.5).
- **P31 (Secret exclusion).** For all repository states, no Jev_Request `state` and no
  report contains content that matches the Secret_Denylist, and no output contains the
  API key (Requirement 9.1, 9.2, 9.6, 9.7).
- **P32 (Error non-panic).** For all Jev error statuses (401, 422, 429, 529), the
  Http_Client returns a typed error and raises no panic (Requirement 10.6, 10.7).
- **P33 (Token-budget bound).** For all built requests, the Jev_Request stays at or less
  than the 32000-token request budget (Requirement 4.10).
