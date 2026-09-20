# Requirements Document

## Introduction

This feature turns the Jev evaluation harness into an active guardrail on the live
request path. The harness from the `jev-integration-eval` spec measures the five aspects
offline. This feature wires two of them, rigor scoring and drift, into a pre-write check
that runs before a change reaches the filesystem. The goal is active prevention, not an
after-the-fact test.

The guardrail has two layers. The deterministic layer runs first, with no Jev call and
regardless of the feature flag. It blocks a write to a protected path and a write whose
content matches the secret denylist. The probabilistic layer runs only when the Jev
feature is on and the API key is present. It scores the change for a hallucinated import,
a convention violation, high complexity, and scope drift, then combines the signals into
one decision: allow, block, or annotate.

The guardrail is delivered through two surfaces. The first surface is a new tool,
`truenorth_guard_change`, that an agent calls with a proposed change before it writes. The
second surface is an emitted `PreToolUse` hook that calls the same logic and returns a pass
or a block for a write tool.

An honest boundary applies. TrueNorth is an MCP server. It cannot intercept a call that an
agent makes to a different tool, such as a client `write_to_file` tool. Automatic
interception of a foreign write tool needs the agent-side `PreToolUse` hook, and it holds
only when the client honors that hook. The explicit `truenorth_guard_change` tool is the
fallback everywhere else. This feature never claims that TrueNorth force-intercepts a
foreign write.

The feature preserves the harness opt-in model. The runtime `jev` flag defaults to off.
The Cargo `jev-http` feature gates the networking dependency. With the defaults, the
probabilistic layer makes no network call, and `cargo test` runs offline against the named
fake. The deterministic layer runs regardless of the flag, because it needs no Jev call.

The probabilistic layer fails open. When Jev is unavailable, the call times out, the
confidence is low, or the flag is off, the probabilistic layer degrades to allow with a
note. It never hard-blocks the developer on a network dependency. The deterministic layer
never fails open: a protected-path hit or a secret hit always blocks.

## Glossary

- **Guardrail**: The active pre-write check that combines the deterministic layer and the
  probabilistic layer into one decision.
- **Deterministic_Layer**: The model-free checks that run with no Jev call: the
  protected-path match and the secret-denylist scan. This layer never fails open.
- **Probabilistic_Layer**: The Jev-scored checks: rigor scoring and the drift Noul. This
  layer fails open.
- **Guard_Decision**: The guardrail result, one of Allow, Block, or Annotate.
- **Neutralization_Packet**: The structured payload a Block returns. It names the violated
  check, the offending value, and a remediation hint, and it can carry a suggested
  self-heal instruction.
- **Proposed_Change**: The input to the guardrail: the target path or paths and the new
  content or a diff the agent intends to write.
- **Guard_Tool**: The `truenorth_guard_change` MCP tool an agent calls before a write.
- **PreToolUse_Hook**: The emitted hook that calls the guardrail logic for a write tool and
  returns an exit code.
- **Jev_Feature_Flag**: The per-project `jev` flag in the `features` block of
  `.agent/config/rules.yml`. The default is off (from `jev-integration-eval`).
- **Protected_Path**: One entry in the `protected_paths` list of `.agent/config/rules.yml`:
  `specs/`, `specs/adr/`, `LICENSE`, and `.github/workflows/`.
- **Secret_Denylist**: The load-bearing pattern list the runtime never reads, echoes, or
  sends outward, reused from `crate::config`.
- **Confidence_Threshold**: A configured cutoff from `JevConfig`. A destructive block uses
  the higher `destructive_threshold`.
- **Fail_Open**: The behavior where the Probabilistic_Layer returns Allow with a note
  rather than a Block when it cannot reach a confident judgment.

## Requirements

### Requirement 1: The active guard tool

**User Story:** As an agent author, I want a guard tool I call before a write, so that a
bad change is caught before it reaches the filesystem.

#### Acceptance Criteria

1. THE Guardrail SHALL expose an MCP tool named `truenorth_guard_change` with a strict `schemars` input schema.
2. THE Guard_Tool input SHALL carry the target path or paths and the new content or a diff for the Proposed_Change.
3. WHEN the Guard_Tool receives a Proposed_Change, THE Guardrail SHALL run the Deterministic_Layer before the Probabilistic_Layer.
4. WHEN the Guardrail reaches a decision, THE Guard_Tool SHALL return one Guard_Decision: Allow, Block, or Annotate.
5. IF a required input field is absent or malformed, THEN THE Guard_Tool SHALL return a typed error naming the offending field and SHALL run no check.
6. THE Guard_Tool SHALL register in the tool router alongside the existing tools and SHALL leave every other tool unchanged.

### Requirement 2: The deterministic layer never fails open

**User Story:** As a security-conscious maintainer, I want the model-free checks to always
block, so that a protected path or a secret is never written on a Jev outage.

#### Acceptance Criteria

1. WHEN a Proposed_Change writes a path that matches a Protected_Path entry, THE Deterministic_Layer SHALL return Block with no Jev call.
2. WHEN a Proposed_Change content matches the Secret_Denylist, THE Deterministic_Layer SHALL return Block with no Jev call.
3. THE Deterministic_Layer SHALL run regardless of the Jev_Feature_Flag value.
4. WHILE the Jev endpoint is unreachable, THE Deterministic_Layer SHALL still return Block for a protected-path or a secret hit.
5. THE Deterministic_Layer SHALL apply the same input and return the same Block or pass result on every evaluation.
6. WHEN the Deterministic_Layer returns Block, THE Neutralization_Packet SHALL name the violated check as the protected-path match or the secret match.

### Requirement 3: The probabilistic layer and its signals

**User Story:** As a maintainer, I want the change scored for drift and rigor, so that a
likely-bad change is flagged with a confidence.

#### Acceptance Criteria

1. WHERE the Jev_Feature_Flag is on and the API key is present, THE Probabilistic_Layer SHALL score the Proposed_Change with the rigor aspect and the drift aspect.
2. THE Probabilistic_Layer SHALL build the Jev state through the secret filter, so no Secret_Denylist content reaches Jev.
3. IF the secret filter reports a residual secret, THEN THE Probabilistic_Layer SHALL make no Jev call and SHALL surface the residual-secret error.
4. WHEN the rigor aspect reports a value at or above the configured boundary, or the drift Noul is at or above the drift boundary, THE Probabilistic_Layer SHALL treat the change as a candidate Block.
5. THE Probabilistic_Layer SHALL keep every Jev request at or under the token budget.

### Requirement 4: The probabilistic layer fails open

**User Story:** As a developer, I want the guard to keep me moving when Jev is down, so
that a network outage never hard-blocks my work.

#### Acceptance Criteria

1. WHILE the Jev_Feature_Flag is off, THE Probabilistic_Layer SHALL make no Jev call and SHALL return Allow with a note that the probabilistic layer did not run.
2. IF the API key is absent, THEN THE Probabilistic_Layer SHALL return Allow with a note and SHALL make no Jev call.
3. IF the Jev call times out, returns a network error, or returns a non-success status, THEN THE Probabilistic_Layer SHALL return Allow with a note naming the cause.
4. IF a candidate Block signal carries a confidence below the Confidence_Threshold, THEN THE Probabilistic_Layer SHALL return Allow with a note rather than Block.
5. THE Probabilistic_Layer SHALL never return Block solely because Jev was unavailable.

### Requirement 5: The confidence gate

**User Story:** As a maintainer, I want a block to require a confident signal, so that a
weak model judgment does not stop a valid change.

#### Acceptance Criteria

1. WHEN a candidate Block signal carries a confidence at or above the Confidence_Threshold, THE Probabilistic_Layer SHALL return Block.
2. WHILE a candidate Block signal carries a confidence below the Confidence_Threshold, THE Probabilistic_Layer SHALL return Allow or Annotate, not Block.
3. WHERE a candidate Block is a destructive action, THE Probabilistic_Layer SHALL apply the higher `destructive_threshold`.
4. THE Probabilistic_Layer SHALL read the thresholds from `JevConfig` and SHALL hardcode no threshold value.
5. THE Guardrail SHALL apply the same signals and the same thresholds and SHALL return the same Guard_Decision.

### Requirement 6: The neutralization packet

**User Story:** As an agent, I want a structured block reason, so that I can re-align the
change without guessing.

#### Acceptance Criteria

1. WHEN the Guardrail returns Block, THE Neutralization_Packet SHALL name the violated check and a remediation hint.
2. WHEN the Block follows a rigor failure, THE Neutralization_Packet SHALL carry a suggested self-heal instruction from the self-heal decision.
3. THE Neutralization_Packet SHALL contain no Secret_Denylist content and no API key.
4. THE Neutralization_Packet SHALL name the offending value for a deterministic block, such as the protected path or the matched pattern name.
5. WHEN the Guardrail returns Annotate, THE result SHALL carry the advisory notes without a Block.

### Requirement 7: The emitted PreToolUse hook

**User Story:** As a project owner, I want an emitted hook that consults the guardrail, so
that a write tool is checked automatically where my client supports it.

#### Acceptance Criteria

1. THE scaffold SHALL emit a `PreToolUse` hook that matches a write tool and calls the guardrail entry point.
2. WHEN the guardrail entry point returns a deterministic Block, THE PreToolUse_Hook SHALL exit with a non-zero status and SHALL write the block reason to standard error.
3. WHEN the guardrail entry point returns Allow, THE PreToolUse_Hook SHALL exit zero.
4. THE emitted hook documentation SHALL state that automatic interception depends on the client honoring the `PreToolUse` hook, and that the Guard_Tool is the fallback.
5. THE emitted hook SHALL make no network call when the Jev_Feature_Flag is off.
6. THE PreToolUse_Hook SHALL leak no API key and no Secret_Denylist content to standard output or standard error.

### Requirement 8: Opt-in and offline behavior

**User Story:** As a runtime operator, I want the active guardrail behind the same opt-in
as the harness, so that the runtime stays offline-friendly.

#### Acceptance Criteria

1. THE Guardrail SHALL read the Jev_Feature_Flag by the same reader that resolves the harness flag.
2. WHILE a test runs, THE Guardrail SHALL use the named fake client and SHALL make no network call.
3. THE Probabilistic_Layer networking SHALL compile only under the Cargo `jev-http` feature.
4. THE Deterministic_Layer SHALL compile and run without the `jev-http` feature.
5. WHERE the `jev-http` feature is off, THE default build SHALL compile no HTTP client for the guardrail.

### Requirement 9: Honest scope boundary

**User Story:** As a reader of the guardrail, I want the boundary stated plainly, so that
nobody expects TrueNorth to intercept a foreign write.

#### Acceptance Criteria

1. THE Guardrail SHALL run per Proposed_Change or per tool call, not per keystroke.
2. THE Guardrail SHALL run no autonomous repair and SHALL trigger no secondary agent.
3. WHEN a rigor failure warrants a fix, THE Guardrail SHALL return the suggested self-heal instruction only, and SHALL not apply it.
4. THE documentation SHALL state that TrueNorth cannot force-intercept a call an agent makes to a foreign tool, and that the `PreToolUse` hook is the automatic path only where the client honors it.

## Correctness Properties

These properties continue the repository numbering from P34. Each suits a property-based
test with the `proptest` crate. The named fake client drives every property test with no
network call.

- **P34 (Deterministic protected-path block).** For all proposed changes that write a
  protected path, the Deterministic_Layer returns Block with no Jev call, regardless of the
  flag (Requirement 2.1, 2.3, 2.4).
- **P35 (Deterministic secret block).** For all proposed changes whose content matches the
  Secret_Denylist, the Deterministic_Layer returns Block and sends nothing to Jev
  (Requirement 2.2, 3.3).
- **P36 (Flag-off silence).** For all proposed changes, while the Jev_Feature_Flag is off,
  the guardrail makes no Jev call and the Probabilistic_Layer returns Allow with a note
  (Requirement 4.1, 8.2).
- **P37 (Probabilistic fail-open).** For all Jev unavailability outcomes (timeout, network
  error, non-success status, absent key), the Probabilistic_Layer returns Allow, never
  Block (Requirement 4.2, 4.3, 4.5).
- **P38 (Confidence-gated block).** For all candidate Block signals and all valid
  thresholds, the Probabilistic_Layer returns Block exactly when the confidence is at or
  above the threshold (Requirement 5.1, 5.2).
- **P39 (Decision determinism).** For all signals and all thresholds, two evaluations of
  the same input return the same Guard_Decision (Requirement 5.5).
- **P40 (No secret to Jev).** For all proposed changes, no Jev request state and no
  Neutralization_Packet contains Secret_Denylist content, and no output contains the API
  key (Requirement 3.2, 6.3).
- **P41 (Hook exit code).** For all guardrail entry-point outcomes, the PreToolUse_Hook
  exits non-zero on a deterministic Block and zero on Allow (Requirement 7.2, 7.3).
