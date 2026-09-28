'use strict';

async function run(ctx) {
  const project = `${ctx.workspace}/project`;
  const oldPackage = `${ctx.workspace}/old-package`;
  const newPackage = `${ctx.workspace}/new-package`;
  await ctx.cli(['init', '--profile', 'generic', '--bundle-dir', oldPackage], { cwd: project });
  ctx.mutate('project/specs/contract.md', 'This authored specification must remain unchanged.\n');
  ctx.mutate(
    'project/.agent/workspace-manifest.yml',
    ctx
      .read('project/.agent/workspace-manifest.yml')
      .replace('bundle_version: 1.0.2', 'bundle_version: 0.9.0'),
  );
  await ctx.git(['init', '-b', 'main'], { cwd: project });
  await ctx.git(['config', 'user.email', 'eval@example.test'], { cwd: project });
  await ctx.git(['config', 'user.name', 'Methodology Eval'], { cwd: project });
  await ctx.git(['add', '.'], { cwd: project });
  await ctx.git(['commit', '-m', 'prior workspace'], { cwd: project });

  ctx.mutate('project/skills/using-truenorth/SKILL.md', 'locally customized workflow\n');
  await ctx.git(['add', 'skills/using-truenorth/SKILL.md'], { cwd: project });
  await ctx.git(['commit', '-m', 'customize workflow'], { cwd: project });

  await ctx.cli(['upgrade', '--check', '--bundle-dir', newPackage], { cwd: project });
  await ctx.cli(['upgrade', '--bundle-dir', newPackage], { cwd: project });
  await ctx.cli(['--check-config'], {
    cwd: project,
    env: { TRUENORTH_VERIFY_CMD: 'true' },
  });
}

module.exports = { run };
