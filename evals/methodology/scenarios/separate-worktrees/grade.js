'use strict';

const fs = require('node:fs');
const path = require('node:path');

const result = (id, passed, evidence) => ({ id, passed, evidence });

function grade({ evidence }) {
  const first = fs.readFileSync(
    path.join(evidence.workspace, '.agent/tasks/release-plan.yml'),
    'utf8',
  );
  const second = fs.readFileSync(
    path.join(evidence.scenario_root, 'agent-two/.agent/tasks/release-plan.yml'),
    'utf8',
  );
  const tools = evidence.events.filter((event) => event.type === 'mcp_tool');
  const conflict = tools.find((event) => event.session === 'contender' && event.ok === false);
  const recovery = tools.find(
    (event) => event.session === 'contender' && event.args.task_name === 'recovered task',
  );
  const branchCommands = evidence.events.filter(
    (event) =>
      event.type === 'command' &&
      event.command === 'git' &&
      JSON.stringify(event.args) === JSON.stringify(['rev-parse', '--abbrev-ref', 'HEAD']),
  );
  const branches = branchCommands.map((event) => event.result.stdout.trim());
  return [
    result(
      'worktrees-branches-distinct',
      new Set(branches).size === 2 && branches.includes('main') && branches.includes('agent-two'),
      `observed branches: ${branches.join(', ')}`,
    ),
    result(
      'worktrees-independent-state',
      first.includes('first worktree task') &&
        first.includes('recovered task') &&
        !first.includes('second worktree task') &&
        second.includes('second worktree task') &&
        !second.includes('first worktree task'),
      'task records remain local to their worktrees',
    ),
    result(
      'worktrees-lease-recovery',
      recovery?.ok === true,
      'contender acquired the lease after owner shutdown',
    ),
    result(
      'worktrees-same-root-conflict',
      conflict?.error?.data?.type === 'writer_lease_conflict',
      `typed conflict: ${conflict?.error?.data?.type ?? 'missing'}`,
    ),
  ];
}

module.exports = { grade };
