'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');

const {
  baselineProjection,
  runProcess,
  stableStringify,
  validateManifest,
} = require('./lib/methodology-eval.js');
const { parseArgs, validateAssertions } = require('./run-methodology-evals.js');

test('parseArgs supports modes, filters, baselines, and explicit binaries', () => {
  const parsed = parseArgs([
    '--mode',
    'deterministic',
    '--scenario',
    'small-feature,bug-fix',
    '--check-baseline',
    '--binary',
    './runtime',
  ]);
  assert.equal(parsed.mode, 'deterministic');
  assert.deepEqual([...parsed.scenarios], ['small-feature', 'bug-fix']);
  assert.equal(parsed.checkBaseline, true);
  assert.equal(parsed.binary, './runtime');
  assert.throws(() => parseArgs(['--check-baseline', '--update-baseline']), /mutually exclusive/);
  assert.throws(() => parseArgs(['--mode', 'unknown']), /unknown mode/);
});

test('validateAssertions enforces the stable assertion API and sorts results', () => {
  const assertions = validateAssertions(
    'fixture',
    [
      { id: 'second', passed: true, evidence: 'two' },
      { id: 'first', passed: false, evidence: 'one' },
    ],
    ['first', 'second'],
  );
  assert.deepEqual(
    assertions.map((entry) => entry.id),
    ['first', 'second'],
  );
  assert.throws(
    () => validateAssertions('fixture', assertions, ['first', 'missing']),
    /assertion ids differ/,
  );
});

test('manifest validation rejects duplicate ids, unknown modes, and unsafe fixtures', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'methodology-manifest-'));
  try {
    const directory = path.join(root, 'scenarios', 'sample');
    fs.mkdirSync(path.join(directory, 'fixture'), { recursive: true });
    fs.writeFileSync(path.join(directory, 'fixture', 'input.txt'), 'fixed\n');
    fs.writeFileSync(path.join(directory, 'run.js'), 'module.exports = { run() {} };\n');
    fs.writeFileSync(
      path.join(directory, 'grade.js'),
      'module.exports = { grade() { return []; } };\n',
    );
    fs.writeFileSync(
      path.join(directory, 'scenario.json'),
      stableStringify({
        schema_version: 1,
        id: 'sample',
        description: 'sample',
        modes: ['deterministic'],
        fixture: 'fixture',
        driver: 'run.js',
        grader: 'grade.js',
        timeout_ms: 100,
        expected_assertions: { deterministic: ['sample-pass'] },
      }),
    );
    assert.equal(
      validateManifest({ schema_version: 1, scenarios: [{ id: 'sample' }] }, root).length,
      1,
    );
    assert.throws(
      () =>
        validateManifest(
          { schema_version: 1, scenarios: [{ id: 'sample' }, { id: 'sample' }] },
          root,
        ),
      /duplicate scenario id/,
    );
    const scenario = JSON.parse(fs.readFileSync(path.join(directory, 'scenario.json'), 'utf8'));
    scenario.modes = ['unknown'];
    fs.writeFileSync(path.join(directory, 'scenario.json'), JSON.stringify(scenario));
    assert.throws(
      () => validateManifest({ schema_version: 1, scenarios: [{ id: 'sample' }] }, root),
      /unknown mode/,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('runProcess hard-times out a child process', async () => {
  await assert.rejects(
    runProcess(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], {
      cwd: process.cwd(),
      timeoutMs: 25,
      label: 'hung child',
    }),
    /hung child timed out/,
  );
});

test('baseline projection excludes volatile metadata and preserves definition digests', () => {
  const report = {
    runtime: { binary_sha256: 'volatile' },
    scenarios: [
      {
        id: 'sample',
        definition_digests: { scenario: 'a', fixture: 'b', driver: 'c', grader: 'd' },
        assertions: [{ id: 'sample-pass', passed: true, evidence: '/tmp/random' }],
      },
    ],
  };
  assert.deepEqual(baselineProjection(report), {
    schema_version: 1,
    expected_scenario_count: 1,
    scenarios: [
      {
        id: 'sample',
        definition_digests: { scenario: 'a', fixture: 'b', driver: 'c', grader: 'd' },
        assertions: [{ id: 'sample-pass', passed: true }],
      },
    ],
  });
});
