'use strict';

const fs = require('node:fs');
const path = require('node:path');

const result = (id, passed, evidence) => ({ id, passed, evidence });
const read = (root, relative) =>
  fs.existsSync(path.join(root, relative))
    ? fs.readFileSync(path.join(root, relative), 'utf8')
    : '';

function grade({ evidence }) {
  const root = evidence.workspace;
  const state = read(root, '.agent/tasks/state.yml');
  const status = read(root, '.agent/tasks/execution-status.yml');
  const requirement = read(root, '.agent/spec/requirements.md');
  const workItem = read(root, '.agent/tasks/normalize-display-name-spec.md');
  const unrelated = read(root, 'unrelated.md');
  return [
    result(
      'requirement-change-active-execution-preserved',
      state.includes('active_task: normalize-display-name') &&
        status.includes('normalize-display-name'),
      'active task remains represented in state and execution status',
    ),
    result(
      'requirement-change-owner-routing',
      /owner:\s*(plan-work|slice-tasks|plan-tests|plan-release)/i.test(workItem),
      'changed work item identifies a declared planning owner',
    ),
    result(
      'requirement-change-revised-verification',
      /trimmed lowercase/i.test(requirement + workItem) && /verify:/i.test(workItem),
      'revised requirement and verification are recorded',
    ),
    result(
      'requirement-change-unrelated-identical',
      unrelated === 'This unrelated requirement must remain byte-identical.\n',
      'unrelated requirement bytes are unchanged',
    ),
  ];
}

module.exports = { grade };
