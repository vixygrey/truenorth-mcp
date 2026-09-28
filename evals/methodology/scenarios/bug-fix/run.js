'use strict';

async function bootstrap(ctx) {
  await ctx.cli(['init', '--profile', 'issue-per-task', '--bundle-dir', `${ctx.repoRoot}/npm`]);
  await ctx.git(['init', '-b', 'main']);
  await ctx.git(['config', 'user.email', 'eval@example.test']);
  await ctx.git(['config', 'user.name', 'Methodology Eval']);
  await ctx.git(['add', '.']);
  await ctx.git(['commit', '-m', 'fixture']);
}

async function run(ctx) {
  await bootstrap(ctx);
  if (ctx.mode === 'model') {
    await ctx.runModel(ctx.scenario.model_prompt);
    return;
  }

  await ctx.command('node', ['regression.js'], {
    expectFailure: true,
    label: 'pre-fix regression',
  });
  await ctx.startSession('bug', { env: { TRUENORTH_VERIFY_CMD: 'node regression.js' } });
  await ctx.callTool('bug', 'truenorth_record_task', {
    group_id: 'BUG-435',
    group_kind: 'ticket',
    task_name: 'Fix normalize regression',
    verify_command: 'node regression.js',
  });
  await ctx.callTool('bug', 'truenorth_record_bug', {
    id: 'fixture-435',
    external_link: 'https://example.test/issues/435',
    status: 'in-progress',
    linked_ref: 'BUG-435',
    tags: ['regression'],
  });
  await ctx.callTool('bug', 'truenorth_tdd_cycle', {
    step: 'red',
    failing_test_cmd: 'node regression.js',
    files_to_modify: ['src/normalize.js'],
  });
  ctx.mutate(
    'src/normalize.js',
    "'use strict';\n\nfunction normalize(value) {\n  return value.trim().toLowerCase();\n}\n\nmodule.exports = { normalize };\n",
  );
  for (const step of ['green', 'refactor']) {
    await ctx.callTool('bug', 'truenorth_tdd_cycle', {
      step,
      failing_test_cmd: 'node regression.js',
      files_to_modify: ['src/normalize.js'],
    });
  }
  await ctx.callTool('bug', 'truenorth_verify_gate', { phase: 'execute' });
  await ctx.command('node', ['regression.js'], { label: 'post-fix regression' });
  await ctx.callTool('bug', 'truenorth_record_bug', {
    id: 'fixture-435',
    external_link: 'https://example.test/issues/435',
    status: 'resolved',
    linked_ref: 'BUG-435',
    tags: ['regression'],
  });
  await ctx.stopSession('bug');
}

module.exports = { run };
