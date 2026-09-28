'use strict';

const fs = require('node:fs');
const path = require('node:path');

function result(id, passed, evidence) {
  return { id, passed, evidence };
}

function grade({ evidence, mode }) {
  const events = evidence.events;
  if (mode === 'model') {
    const statusPath = path.join(evidence.workspace, '.agent/tasks/execution-status.yml');
    const statePath = path.join(evidence.workspace, '.agent/tasks/state.yml');
    const status = fs.existsSync(statusPath) ? fs.readFileSync(statusPath, 'utf8') : '';
    const state = fs.existsSync(statePath) ? fs.readFileSync(statePath, 'utf8') : '';
    const verification = events.some((event) => event.type === 'model' && event.result.code === 0);
    return [
      result(
        'small-feature-model-execution-status',
        /status:\s*done/.test(status),
        'task status is done',
      ),
      result(
        'small-feature-model-handoff',
        /last_step_completed:|next_skill:/.test(state),
        'handoff is recorded',
      ),
      result(
        'small-feature-model-verification',
        verification,
        'configured agent command exited zero',
      ),
    ];
  }

  const tools = events.filter((event) => event.type === 'mcp_tool');
  const phases = tools
    .filter((event) => event.tool === 'truenorth_advance_phase')
    .map((event) => event.args.to_phase);
  const tdd = tools
    .filter((event) => event.tool === 'truenorth_tdd_cycle')
    .map((event) => event.args.step);
  const plan = fs.readFileSync(
    path.join(evidence.workspace, '.agent/tasks/release-plan.yml'),
    'utf8',
  );
  const red = tools.find(
    (event) => event.tool === 'truenorth_tdd_cycle' && event.args.step === 'red',
  );
  const gate = tools.find((event) => event.tool === 'truenorth_verify_gate');
  return [
    result(
      'small-feature-gate-pass',
      gate?.ok === true,
      gate?.ok ? 'real verification gate passed' : 'verification gate did not pass',
    ),
    result(
      'small-feature-phase-order',
      JSON.stringify(phases) ===
        JSON.stringify(['design', 'plan', 'execute', 'review', 'integrate']),
      `observed phases: ${phases.join(', ')}`,
    ),
    result(
      'small-feature-red-observed',
      red?.ok === true,
      'red tool accepted a real non-zero test command',
    ),
    result(
      'small-feature-task-record',
      plan.includes('Implement add behavior') &&
        plan.includes('435-fixture') &&
        plan.includes('node test.js'),
      'release plan contains task, grouping, and verify command',
    ),
    result(
      'small-feature-tdd-order',
      JSON.stringify(tdd) === JSON.stringify(['red', 'green', 'refactor']),
      `observed TDD steps: ${tdd.join(', ')}`,
    ),
  ];
}

module.exports = { grade };
