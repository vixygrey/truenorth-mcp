'use strict';

const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');

const { buildManifest, serialized } = require('./build-bundle-manifest.js');

const root = path.resolve(__dirname, '..');

test('current bundle manifest is deterministic and matches tracked skills', () => {
  const manifest = buildManifest(root);
  const paths = manifest.files.map((entry) => entry.path);
  assert.deepStrictEqual(paths, [...paths].sort());
  assert.strictEqual(new Set(paths).size, paths.length);
  assert.ok(paths.includes('skills/using-truenorth/SKILL.md'));
  assert.ok(paths.every((entry) => !entry.endsWith('.DS_Store')));
  assert.ok(manifest.files.every((entry) => /^[a-f0-9]{64}$/.test(entry.sha256)));
  assert.ok(manifest.files.every((entry) => entry.mode === '0644' || entry.mode === '0755'));
  assert.strictEqual(manifest.schema_version, 2);
  assert.strictEqual(manifest.workspace_schema_version, '2');
  assert.deepStrictEqual(
    manifest.skill_sets.map((set) => set.name),
    ['core', 'integrations', 'maintainer', 'visual'],
  );
  assert.strictEqual(manifest.skill_sets.find((set) => set.name === 'core').skills.length, 70);
  assert.strictEqual(manifest.skill_sets.flatMap((set) => set.skills).length, 76);
});

test('checked-in current bundle manifest is current', async () => {
  const expected = await serialized(buildManifest(root));
  const actual = fs.readFileSync(path.join(root, 'npm/bundle/current.json'), 'utf8');
  assert.strictEqual(actual, expected);
});
