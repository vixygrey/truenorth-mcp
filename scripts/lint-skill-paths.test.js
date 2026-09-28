'use strict';

const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync, spawnSync } = require('node:child_process');
const { afterEach, test } = require('node:test');

const lint = path.resolve(__dirname, 'lint-skill-paths.sh');
const roots = [];

afterEach(() => {
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function fixture(relativePath, content) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-skill-paths-'));
  roots.push(root);
  const target = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content);
  execFileSync('git', ['init', '--quiet'], { cwd: root });
  execFileSync('git', ['add', relativePath], { cwd: root });
  return root;
}

function run(root) {
  return spawnSync('bash', [lint, '--root', root], {
    cwd: root,
    encoding: 'utf8',
  });
}

test('rejects an unqualified cockpit filename in a source skill', () => {
  const root = fixture('skills/example/SKILL.md', 'Write the handoff to `state.yaml`.\n');
  const result = run(root);
  assert.notStrictEqual(result.status, 0);
  assert.match(result.stderr, /bare cockpit filename/);
  assert.match(result.stderr, /skills\/example\/SKILL\.md:1/);
});

test('accepts the canonical qualified cockpit path', () => {
  const root = fixture(
    'skills/example/SKILL.md',
    'Write the handoff to `.agent/tasks/state.yml`.\n',
  );
  const result = run(root);
  assert.strictEqual(result.status, 0, result.stderr);
});

test('accepts an explicitly marked legacy migration source example', () => {
  const root = fixture(
    'skills/migrate-spec/SKILL.md',
    'Legacy source input: `state.yaml`. <!-- truenorth-lint: allow-legacy-cockpit-path -->\n',
  );
  const result = run(root);
  assert.strictEqual(result.status, 0, result.stderr);
});

test('rejects an unmarked stale output path inside migrate-spec', () => {
  const root = fixture(
    'skills/migrate-spec/SKILL.md',
    'Generate `state.yaml` as the TrueNorth output.\n',
  );
  const result = run(root);
  assert.notStrictEqual(result.status, 0);
  assert.match(result.stderr, /bare cockpit filename/);
});

test('checks in-repo wiki pages', () => {
  const root = fixture('wiki/Skills-Plan.md', 'Read `release-plan.yaml`.\n');
  const result = run(root);
  assert.notStrictEqual(result.status, 0);
  assert.match(result.stderr, /wiki\/Skills-Plan\.md:1/);
});
