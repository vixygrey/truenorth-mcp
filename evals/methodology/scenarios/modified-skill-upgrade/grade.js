'use strict';

const fs = require('node:fs');
const path = require('node:path');

const result = (id, passed, evidence) => ({ id, passed, evidence });

function grade({ evidence }) {
  const project = path.join(evidence.workspace, 'project');
  const cli = evidence.events.filter((event) => event.type === 'cli');
  const check = cli.find((event) => event.args[0] === 'upgrade' && event.args.includes('--check'));
  const apply = cli.find((event) => event.args[0] === 'upgrade' && !event.args.includes('--check'));
  const config = cli.find((event) => event.args[0] === '--check-config');
  const plan = check ? JSON.parse(check.result.stdout) : null;
  const local = fs.readFileSync(path.join(project, 'skills/using-truenorth/SKILL.md'), 'utf8');
  const stable = fs.readFileSync(path.join(project, 'skills/stable-skill/SKILL.md'), 'utf8');
  const manifest = fs.readFileSync(path.join(project, '.agent/workspace-manifest.yml'), 'utf8');
  const spec = fs.readFileSync(path.join(project, 'specs/contract.md'), 'utf8');
  const conflict = plan?.actions?.find((entry) => entry.path === 'skills/using-truenorth/SKILL.md');
  return [
    result(
      'upgrade-check-conflict',
      conflict?.action === 'conflict',
      `check action: ${conflict?.action ?? 'missing'}`,
    ),
    result(
      'upgrade-check-read-only',
      check?.result.code === 0 && apply?.index > check?.index,
      'check completed before apply without mutation failure',
    ),
    result(
      'upgrade-configuration-valid',
      config?.result.code === 0,
      'configuration check exited zero',
    ),
    result(
      'upgrade-local-skill-preserved',
      local === 'locally customized workflow\n',
      'local skill bytes are preserved',
    ),
    result(
      'upgrade-manifest-advanced',
      manifest.includes('bundle_version: 1.0.3') &&
        manifest.includes(conflict ? conflict.path : 'missing'),
      'workspace manifest advances the target bundle baseline',
    ),
    result(
      'upgrade-specs-unchanged',
      spec === 'This authored specification must remain unchanged.\n',
      'authored specs bytes are unchanged',
    ),
    result(
      'upgrade-stable-skill-updated',
      stable === 'new stable workflow\n',
      'unchanged managed skill updated to target bytes',
    ),
    result(
      'upgrade-transaction-cleaned',
      !fs.existsSync(path.join(project, '.agent/runtime/upgrade')),
      'upgrade transaction directory is absent',
    ),
  ];
}

module.exports = { grade };
