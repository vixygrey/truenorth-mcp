'use strict';

async function run(ctx) {
  await ctx.cli(['init', '--profile', 'issue-per-task', '--bundle-dir', `${ctx.repoRoot}/npm`]);
  await ctx.git(['init', '-b', 'main']);
  await ctx.git(['config', 'user.email', 'eval@example.test']);
  await ctx.git(['config', 'user.name', 'Methodology Eval']);
  await ctx.git(['add', '.']);
  await ctx.git(['commit', '-m', 'fixture']);

  const second = `${ctx.scenarioRoot}/agent-two`;
  await ctx.git(['worktree', 'add', '-b', 'agent-two', second]);
  await ctx.startSession('first', { root: ctx.workspace });
  await ctx.startSession('second', { root: second });
  await ctx.callTool('first', 'truenorth_record_task', {
    task_name: 'first worktree task',
    verify_command: 'true',
  });
  await ctx.callTool('second', 'truenorth_record_task', {
    task_name: 'second worktree task',
    verify_command: 'true',
  });

  await ctx.startSession('contender', { root: ctx.workspace });
  await ctx.callTool(
    'contender',
    'truenorth_record_task',
    { task_name: 'blocked task', verify_command: 'true' },
    { expectError: true },
  );
  await ctx.stopSession('first');
  await ctx.callTool('contender', 'truenorth_record_task', {
    task_name: 'recovered task',
    verify_command: 'true',
  });
  await ctx.stopSession('contender');
  await ctx.stopSession('second');

  await ctx.git(['add', '.agent/tasks'], { cwd: ctx.workspace });
  await ctx.git(['commit', '-m', 'first worktree state'], { cwd: ctx.workspace });
  await ctx.git(['add', '.agent/tasks'], { cwd: second });
  await ctx.git(['commit', '-m', 'second worktree state'], { cwd: second });
  await ctx.git(['rev-parse', '--abbrev-ref', 'HEAD'], { cwd: ctx.workspace });
  await ctx.git(['rev-parse', '--abbrev-ref', 'HEAD'], { cwd: second });
}

module.exports = { run };
