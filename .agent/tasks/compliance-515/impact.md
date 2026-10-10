# Compliance remediation impact assessment

## Target

Shared release/planning skills, their npm mirrors and catalog ownership; authoritative requirements/writing references; verification/coverage workflow; cockpit evidence; remote integration controls. This assessment precedes detailed work-item plans. No exported Rust/JavaScript symbol change is proposed by the planning bootstrap.

## Dependents

The checked-in skills/catalog.yml lists the following consumers of planning artifacts: assess-impact, execute-group, change-request, grill-me, plan-work, slice-tasks, survey-context, trace-requirement, using-truenorth, visual-dashboard, plan-tests, session-state, orchestrate-project, verify-work, develop-tdd, and release-branch. Source skills also have packaged copies under npm/skills/; npm/bundle/current.json digests those inputs.

- #516 affects release policy, artifact resolution, downstream planning readers, packaged skill consumers, and bundle consistency. Risk: High. Shared contracts have more than ten declared consumers and prose behavior is not established by catalog tests alone.
- #517 affects requirement/design readers and contributors through .agent/spec/requirements.md, .agent/product/scope.yml, .agent/product/glossary.yml, CONVENTIONS.md, CONTRIBUTING.md, and .kiro/steering/authority.md. Risk: High. Shared normative references are absent and require provenance review; risk classification of the work item remains P3 because it changes documentation, not runtime behavior.
- #518 affects the actual client/server gate environment, package.json, scripts/preflight.sh, .github/workflows/ci.yml, and release coverage evidence. Risk: High. The release boundary and policy measurement need real execution and negative-path proof.
- #519 affects execution-status.yml, every release-plan entry, task capsules, traceability consumers, and GitHub historical evidence. Risk: High. Mistaken completion or fabricated evidence can produce a false release verdict.
- #520 affects remote protected-main rules, required checks, integration approvals, and immutable-release evidence. Risk: High. External controls and irreversible operations require remote observations and separate approval.

## Existing verification surfaces

- scripts/build-bundle-manifest.test.js and scripts/build-bundle-manifest.js --check: structural bundle/catalog verification, not proof that a prose workflow was followed.
- scripts/preflight.sh wrapper/shell/format: packaged wrapper, scripted skills, lint, and document formatting checks.
- scripts/run-methodology-evals.js deterministic mode: disposable real CLI/MCP runtime scenarios. The suite explicitly does not claim its fixture mutations prove prose-skill performance.
- Strict diagnostics: actual repository layout/configuration/backlog readiness, not a successful verify gate or coverage measurement.

## Gaps and early detection

- Release coverage namespace and commit-policy disagreements: #516 must exercise current configuration and a convention-compliant message in the real operator flow.
- Normative content recovery: #517 must identify reachable originals or label reviewed reconstruction; empty-file link repairs are prohibited.
- Coverage tool, metric, and business-logic source mapping: #518 must finalize these before implementation and obtain approval for any dependency addition.
- Historical evidence: #519 must retain missing evidence as gaps, never automatic waivers.
- Remote enforcement: #520 must query actual settings and observe checks; YAML is insufficient.

## Recommended action

Proceed with the detailed planning for #516 first. Later tasks remain gated by their declared dependencies and readiness decisions. Run a green baseline and the applicable risk review before implementation. No runtime API change, new abstraction, new dependency, or operating-system containment claim is needed for the planning bootstrap.
