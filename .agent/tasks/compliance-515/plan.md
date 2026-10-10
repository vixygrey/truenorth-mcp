# TrueNorth compliance remediation: issue #515

## Authority and scope

- Parent: https://github.com/vixygrey/truenorth-mcp/issues/515
- Approved plan: /Users/greyvixfen/Desktop/truenorth-methodology-compliance-plan.md
- Product scope: .agent/product/scope.yml
- Profile: issue-per-task. Optional grouping: ticket 515.
- Backlog owner: GitHub. Integration mode: team-pr.
- Planning branch: chore/515-compliance-planning. No implementation or commits have occurred.

## Work items and order

- #516: fix(methodology): reconcile release and planning contracts; risk P1; dependencies planning bootstrap #515; specification 516-spec.md; ledger 516-tasks.yml.
- #517: docs(methodology): restore specification and writing authority; risk P3; dependencies #516; specification 517-spec.md; ledger 517-tasks.yml.
- #518: ci(methodology): establish real verification and coverage gates; risk P1; dependencies #516; specification 518-spec.md; ledger 518-tasks.yml.
- #519: chore(methodology): establish truthful execution and historical evidence; risk P1; dependencies #517, #518; specification 519-spec.md; ledger 519-tasks.yml.
- #520: test(methodology): prove compliant workflow and GitHub controls; risk P0; dependencies #519; specification 520-spec.md; ledger 520-tasks.yml.

## Artifact ownership

- scope-work: .agent/product/scope.yml; preserves existing product requirements.
- truenorth_record_task: runtime-owned release-plan tasks; all five calls returned 1.0.4 mutation receipts.
- plan-release: optional release-index group metadata, including capsule_dir; does not rewrite tasks.
- slice-tasks: group.yml and the reserved specification/ledger filenames.
- plan-tests: test-plan.md; declared issue ids are scenario owners.
- assess-impact: impact.md; maps shared consumers before detailed plans.
- plan-work: each reserved specification and ledger; never execution status.
- truenorth_advance_phase: adjacent lifecycle transitions and observed git context.
- verify-work/execute-group: future verification evidence and execution status, not planning promises.
- trace-requirement: future .agent/tasks/traceability.yml.

## Decisions

1. Use one optional ticket group linked to the planning issue; each work item retains its own actual GitHub issue id. This uses supported group_id/group_kind arguments and existing groups/capsule_dir metadata, not new runtime fields.
2. Keep CONVENTIONS.md authoritative; repair conflicting skill consumers rather than changing valid project policy to match them.
3. Preserve configured coverage thresholds. The coverage metric, tool, source-set mapping, and exact command are a planning gate owned by #518 before its evaluator implementation. No dependency is approved by this document.
4. Preserve existing records and user edits. No task is marked implemented, no completion timestamp is fabricated, and no historical evidence is inferred.
5. Human-authored narrative restoration in specs/ requires its content/provenance review. Runtime and skill writes stay under .agent/.
6. Contract remediation is the first implementation item. Later items cannot start until their dependency and readiness gates pass.

## Execution gates

The planning bootstrap may finish without implementing any remediation. All task ledgers start at status failing (unverified, not an observed failing test). Handoff selects #516 and kickoff-branch. That skill must establish the issue-linked implementation branch and green baseline before implementation; planning approval does not authorize a commit, merge, tag, publication, cleanup, or dependency addition.

## Verification and evidence

Planning verification checks parseability, cross-artifact identity, reserved filenames, scenario resolution, dependency order, real GitHub issue existence, supported release-plan records, and preservation of protected cockpit inputs. Strict diagnostics establish configuration readiness only. Live MCP resource reads prove the server consumes the updated cockpit. Their observed results are retained in planning-evidence.json and do not constitute implementation verification or a historical-compliance verdict.

## Scope boundary

No runtime redesign, new telemetry, ontology enablement, retired placeholder restoration, fabricated history, changed published 1.0.4 asset, or automatic publication. Root issue #515 remains open as the initiative tracker until all required work is complete.
