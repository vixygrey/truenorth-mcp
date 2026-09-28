'use strict';

const fs = require('node:fs');
const path = require('node:path');

const result = (id, passed, evidence) => ({ id, passed, evidence });

function grade({ evidence, mode }) {
  const events = evidence.events;
  if (mode === 'model') {
    const state = readOptional(path.join(evidence.workspace, '.agent/tasks/state.yml'));
    const bugs = readOptional(path.join(evidence.workspace, '.agent/tasks/bugs.yml'));
    const modelPassed = events.some((event) => event.type === 'model' && event.result.code === 0);
    return [
      result(
        'bug-fix-model-bug-cycle',
        /bug_cycle:|active_flow:\s*fix_bug/.test(state),
        'bug flow state is recorded',
      ),
      result('bug-fix-model-resolution', /resolved|closed/.test(bugs), 'bug reference is resolved'),
      result('bug-fix-model-verification', modelPassed, 'configured agent command exited zero'),
    ];
  }

  const tools = events.filter((event) => event.type === 'mcp_tool');
  const commands = events.filter((event) => event.type === 'command');
  const bugs = readOptional(path.join(evidence.workspace, '.agent/tasks/bugs.yml'));
  const tdd = tools
    .filter((event) => event.tool === 'truenorth_tdd_cycle')
    .map((event) => event.args.step);
  const before = commands.find((event) => event.label === 'pre-fix regression');
  const after = commands.find((event) => event.label === 'post-fix regression');
  const gate = tools.find((event) => event.tool === 'truenorth_verify_gate');
  return [
    result(
      'bug-fix-bug-reference',
      bugs.includes('fixture-435') && bugs.includes('resolved') && bugs.includes('BUG-435'),
      'resolved bug reference retains identity and task linkage',
    ),
    result('bug-fix-gate-pass', gate?.ok === true, 'real verification gate passed'),
    result(
      'bug-fix-original-failure',
      before?.result.code !== 0,
      `pre-fix exit code: ${before?.result.code}`,
    ),
    result(
      'bug-fix-regression-pass',
      after?.result.code === 0,
      `post-fix exit code: ${after?.result.code}`,
    ),
    result(
      'bug-fix-tdd-order',
      JSON.stringify(tdd) === JSON.stringify(['red', 'green', 'refactor']),
      `observed TDD steps: ${tdd.join(', ')}`,
    ),
  ];
}

function readOptional(file) {
  return fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
}

module.exports = { grade };
