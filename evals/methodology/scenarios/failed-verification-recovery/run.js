'use strict';

async function run(ctx) {
  await ctx.cli(['init', '--profile', 'generic', '--bundle-dir', `${ctx.repoRoot}/npm`]);
  await ctx.git(['init', '-b', 'main']);
  await ctx.git(['config', 'user.email', 'eval@example.test']);
  await ctx.git(['config', 'user.name', 'Methodology Eval']);
  await ctx.git(['add', '.']);
  await ctx.git(['commit', '-m', 'fixture']);
  await ctx.startSession('recovery', { env: { TRUENORTH_VERIFY_CMD: 'node verify.js' } });
  await ctx.callTool(
    'recovery',
    'truenorth_verify_gate',
    { phase: 'execute' },
    { expectError: true },
  );
  ctx.mutate('status.txt', 'fixed\n');
  await ctx.callTool('recovery', 'truenorth_verify_gate', { phase: 'execute' });
  await ctx.stopSession('recovery');
}

module.exports = { run };
