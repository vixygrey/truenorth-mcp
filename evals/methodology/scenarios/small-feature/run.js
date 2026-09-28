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

  await ctx.startSession('feature', { env: { TRUENORTH_VERIFY_CMD: 'node test.js' } });
  await ctx.callTool('feature', 'truenorth_record_task', {
    group_id: '435-fixture',
    group_kind: 'ticket',
    task_name: 'Implement add behavior',
    verify_command: 'node test.js',
  });
  for (const [from_phase, to_phase] of [
    ['discover', 'design'],
    ['design', 'plan'],
    ['plan', 'execute'],
  ]) {
    await ctx.callTool('feature', 'truenorth_advance_phase', {
      from_phase,
      to_phase,
      artifacts_summary: `Completed ${from_phase}`,
    });
  }
  await ctx.callTool('feature', 'truenorth_tdd_cycle', {
    step: 'red',
    failing_test_cmd: 'node test.js',
    files_to_modify: ['src/add.js'],
  });
  ctx.mutate(
    'src/add.js',
    "'use strict';\n\nfunction add(left, right) {\n  return left + right;\n}\n\nmodule.exports = { add };\n",
  );
  for (const step of ['green', 'refactor']) {
    await ctx.callTool('feature', 'truenorth_tdd_cycle', {
      step,
      failing_test_cmd: 'node test.js',
      files_to_modify: ['src/add.js'],
    });
  }
  await ctx.callTool('feature', 'truenorth_verify_gate', { phase: 'execute' });
  for (const [from_phase, to_phase] of [
    ['execute', 'review'],
    ['review', 'integrate'],
  ]) {
    await ctx.callTool('feature', 'truenorth_advance_phase', {
      from_phase,
      to_phase,
      artifacts_summary: `Completed ${from_phase}`,
    });
  }
  await ctx.stopSession('feature');
}

module.exports = { run };
