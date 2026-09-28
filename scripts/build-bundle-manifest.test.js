'use strict';

const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');

const {
  buildManifest,
  parseCatalog,
  serialized,
  validatePlanningArtifacts,
} = require('./build-bundle-manifest.js');

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
  assert.strictEqual(manifest.skill_sets.find((set) => set.name === 'core').skills.length, 68);
  assert.strictEqual(manifest.skill_sets.flatMap((set) => set.skills).length, 74);
});

test('planning artifacts have one canonical writer and path', () => {
  const catalog = parseCatalog(root);
  const ownership = Object.fromEntries(
    Object.entries(catalog.planning_artifacts).map(([name, artifact]) => [
      name,
      { path: artifact.path, writer: artifact.writer },
    ]),
  );
  assert.deepStrictEqual(ownership, {
    'release-index': {
      path: '.agent/tasks/release-plan.yml',
      writer: 'plan-release',
    },
    'group-manifest': {
      path: '.agent/tasks/<capsule>/group.yml',
      writer: 'slice-tasks',
    },
    'group-test-plan': {
      path: '.agent/tasks/<capsule>/test-plan.md',
      writer: 'plan-tests',
    },
    'work-item-specification': {
      path: '.agent/tasks/<capsule>/<task-id>-spec.md',
      writer: 'plan-work',
    },
    'work-item-task-ledger': {
      path: '.agent/tasks/<capsule>/<task-id>-tasks.yml',
      writer: 'plan-work',
    },
  });
});

test('planning artifact validation rejects ownership conflicts', () => {
  const catalog = parseCatalog(root);
  const duplicatePath = structuredClone(catalog);
  duplicatePath.planning_artifacts['group-test-plan'].path =
    duplicatePath.planning_artifacts['group-manifest'].path;
  assert.throws(
    () => validatePlanningArtifacts(duplicatePath),
    /planning artifacts group-manifest and group-test-plan share path/,
  );

  const missingWriter = structuredClone(catalog);
  delete missingWriter.planning_artifacts['work-item-specification'].writer;
  assert.throws(
    () => validatePlanningArtifacts(missingWriter),
    /planning artifact work-item-specification is missing writer/,
  );

  const missingArtifact = structuredClone(catalog);
  delete missingArtifact.planning_artifacts['group-test-plan'];
  assert.throws(
    () => validatePlanningArtifacts(missingArtifact),
    /planning artifacts must declare exactly the required ownership contract/,
  );
});

test('planning artifact validation rejects unknown or duplicate readers', () => {
  const catalog = parseCatalog(root);
  const unknownReader = structuredClone(catalog);
  unknownReader.planning_artifacts['release-index'].readers.push('missing-skill');
  assert.throws(
    () => validatePlanningArtifacts(unknownReader),
    /planning artifact release-index names unknown reader missing-skill/,
  );

  const duplicateReader = structuredClone(catalog);
  duplicateReader.planning_artifacts['group-manifest'].readers.push('plan-work');
  assert.throws(
    () => validatePlanningArtifacts(duplicateReader),
    /planning artifact group-manifest repeats reader plan-work/,
  );

  const writerReader = structuredClone(catalog);
  writerReader.planning_artifacts['work-item-task-ledger'].readers.push('plan-work');
  assert.throws(
    () => validatePlanningArtifacts(writerReader),
    /planning artifact work-item-task-ledger lists its writer as a reader/,
  );
});

test('checked-in current bundle manifest is current', async () => {
  const expected = await serialized(buildManifest(root));
  const actual = fs.readFileSync(path.join(root, 'npm/bundle/current.json'), 'utf8');
  assert.strictEqual(actual, expected);
});
