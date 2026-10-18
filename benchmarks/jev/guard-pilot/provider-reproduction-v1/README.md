# TypeSafe task-fit review: unresolved Rust import

Status: draft for owner review. Not sent to TypeSafe. No further paid run is authorized by this package.

## Question for TypeSafe

Can `jev-1.13.0` reliably assess unresolved Rust imports from a complete proposed crate root and its Cargo manifest?
Our advisory guard requires a Noul of at least 0.70 to flag this finding.
Two separately authorized pilot observations missed that boundary on the synthetic case in this package.
We request a task-fit assessment, not a guarantee of exact scores or a lower threshold.

1. Is this compiler-resolution task appropriate for Jev 1.13, or outside its intended common-sense judgment scope?
2. Do these instructions and criteria correctly distinguish proposed source from existing source and unavailable evidence?
3. Is there a documented input representation or supported model that better fits this task?

Please review the supplied evidence before suggesting further paid experiments.
Do not run either request on our behalf without separate authorization.

## Contents

- `case.json`: synthetic initial files, proposal, task, and expected result from `pilot-v2-01`.
- `request-before.json`: reconstructed rigor request with the prior import question.
- `request-after.json`: reconstructed rigor request with the clarified import question and Noul criteria.
- `pilot-before.json` and `pilot-after.json`: normalized copies of sanitized runtime reports, not raw provider responses.
- `compiler-evidence.json`: actual offline compiler observations from Rust 1.88.0.
- `verify.js`: standalone offline request-consistency and compiler verifier, using Node 18 or newer and installed Rust.
- `SHA256SUMS`: integrity hashes for the package files.

The request bodies contain all four original rigor questions.
Only the import question differs between the variants.
They contain no authentication header, API key, private endpoint, customer data, or real repository source.
The case is repository-authored synthetic material under this repository's MIT license.

## Observed behavior

| Measure                   | Prior question | Clarified question |
| ------------------------- | -------------- | ------------------ |
| Import Noul               | 0.47           | 0.68               |
| Operating boundary        | 0.70           | 0.70               |
| Expected decision         | annotate       | annotate           |
| Observed decision         | allow          | allow              |
| Concrete model            | jev-1.13.0     | jev-1.13.0         |
| Input tokens, whole case  | 1161           | 1241               |
| Output tokens, whole case | 97             | 97                 |
| HTTP attempts, whole case | 2              | 2                  |
| Estimated cost, USD       | 0.000048762    | 0.000052122        |

The baseline compiles. The complete proposed crate root fails with Rust diagnostic `E0432` for `absent_codec`.
The manifest explicitly selects `src/check.rs` as the library crate root.
Neither the manifest nor the proposed root declares the imported crate or a local module.
The current source is only context, not the proposed replacement.
`CONVENTIONS.md` is absent; the import label does not depend on a project convention.

Each pilot stopped at the first finding or decision mismatch.
The remaining nine cases did not run.
Both reports record complete usage, normal process exit, and available model findings.
Costs are estimates, not verified billing.
This single comparison does not establish statistical improvement or broader model accuracy.

## Offline verification

Run these commands from this directory:

```sh
shasum -a 256 -c SHA256SUMS
node verify.js
```

The verifier never calls the provider or reads an API key.
It materializes only the fixed synthetic source in a disposable directory.
Cargo runs offline with an isolated Cargo home and a restricted environment.
The baseline must compile; the proposal must fail specifically on the unresolved import.
The temporary project is removed afterward.
The verifier also confirms that both requests share the same state and all three unrelated questions.
It does not validate provider accuracy or reproduce a live score.

## Provenance

Prior pilot: https://github.com/vixygrey/truenorth-mcp/actions/runs/39138054756

- Code: `00d0dff9ae66ecba27e66b21ba0007cce255b542`
- Rigor-source SHA-256: `2cfc93425fcd7d99b5a1c4c1e4c2f04f097739e1e02458e832a66ba44c39cad3`

Clarified pilot: https://github.com/vixygrey/truenorth-mcp/actions/runs/39142575391

- Code: `bc6406d22e46df60df2f37a2a2513984c42a2d9d`
- Rigor-source SHA-256: `e4986b4feb36905253ebaa75e06fd1a9ccd04ffc43a7b3ef9320567fee150737`

The reports carry fixture, rationale, policy, configuration, binary, and prompt hashes.
Both use fixture version 2, schema version 2, retry limit zero, and the unchanged pending policy.
The prior source is available at the recorded Git revision.

## Reconstruction and limits

The raw paid HTTP requests and provider response bodies were not retained.
The after request was reconstructed through the production guard builder with the synthetic case and offline FakeClient.
The before request replaces only its import question with the question from the recorded prior source revision.
Neither request file is represented as a captured paid wire payload.
Whitespace and serialization order in these JSON files are normalized.
The aggregate report copies preserve parsed values, not necessarily original byte formatting.

The request files cover rigor only.
Whole-case usage includes a separate scope evaluation, whose request is not included here.
Reports expose the typed runtime findings; they are not complete API `answers` envelopes.
Complexity in the reports uses the guard's transformed 0-to-100 scale.
No missing raw answer or provider request identifier has been invented.

A separate keyless inspection exercised the real HTTP sender, decoder, guard, and MCP result mapper against loopback controls.
Import values 0.47, 0.6999, 0.70, and 0.95 survived unchanged.
The expected decision transition occurred at 0.70.
Malformed, missing, wrong-type, and out-of-range answers remained explicitly unavailable.
That inspection found no defect in the exercised adapter path, not proof that every path is correct.
Its evidence is recorded at https://github.com/vixygrey/truenorth-mcp/issues/344#issuecomment-6251554606.

Relevant provider references:

- https://docs.typesafe.ai/api
- https://docs.typesafe.ai/primitives/noul
- https://docs.typesafe.ai/model-jaggedness/jev-1.13

No threshold or expected label was changed to obtain a pass.
Readiness remains unaccepted. Jev remains advisory and disabled by default.
Owner review is required before sharing this package or selecting another experiment.
