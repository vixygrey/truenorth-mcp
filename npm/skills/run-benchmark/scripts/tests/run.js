'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { test } = require('node:test');

const SKILL_ROOT = path.resolve(__dirname, '../..');
const RUNNER = path.join(SKILL_ROOT, 'scripts', 'run.js');
const DEFINITION = path.join(SKILL_ROOT, 'definitions', 'sample', 'benchmark.json');
const SKILL = path.join(SKILL_ROOT, 'definitions', 'sample', 'SKILL.md');

function invoke(args, cwd = process.cwd()) {
  return spawnSync(process.execPath, [RUNNER, ...args], {
    cwd,
    encoding: 'utf8',
    env: process.env,
  });
}

function runSample(extra = []) {
  return invoke(['--definition', DEFINITION, '--skill', SKILL, ...extra]);
}

test('bundled benchmark executes isolated train and validation runs', () => {
  const result = runSample();
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.schema_version, 1);
  assert.equal(report.skill, 'sample-normalizer');
  assert.equal(report.runs_per_scenario, 2);
  assert.deepEqual(report.train, {
    with_skill: 1,
    without_skill: 0,
    delta: 1,
    scenario_ids: ['trim-and-lowercase'],
  });
  assert.deepEqual(report.validation, {
    with_skill: 1,
    without_skill: 0,
    delta: 1,
    scenario_ids: ['held-out-normalization'],
  });
  assert.equal(report.scenarios.length, 2);
  for (const scenario of report.scenarios) {
    assert.equal(scenario.with_pass_rate, 1);
    assert.equal(scenario.without_pass_rate, 0);
    assert.equal(scenario.delta, 1);
    assert.equal(scenario.runs.with_skill.length, 2);
    assert.equal(scenario.runs.without_skill.length, 2);
    assert(scenario.runs.with_skill.every((run) => run.passed));
    assert(scenario.runs.without_skill.every((run) => !run.passed));
  }
});

test('equivalent benchmark runs emit byte-identical reports', () => {
  const first = runSample();
  const second = runSample();
  assert.equal(first.status, 0, first.stderr);
  assert.equal(second.status, 0, second.stderr);
  assert.equal(first.stdout, second.stdout);
});

test('output and baseline modes use the same versioned report schema', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-skill-benchmark-baseline-'));
  try {
    const reportPath = path.join(root, 'report.json');
    const baselinePath = path.join(root, 'baseline.json');
    let result = runSample(['--output', reportPath, '--update-baseline', baselinePath]);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(JSON.parse(fs.readFileSync(reportPath, 'utf8')), JSON.parse(result.stdout));
    assert.deepEqual(
      JSON.parse(fs.readFileSync(baselinePath, 'utf8')),
      JSON.parse(fs.readFileSync(reportPath, 'utf8')),
    );

    result = runSample(['--check-baseline', baselinePath]);
    assert.equal(result.status, 0, result.stderr);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
test('executor timeouts remain failed run evidence', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-skill-benchmark-timeout-'));
  try {
    for (const split of ['train', 'validation']) fs.mkdirSync(path.join(root, split));
    const definitionPath = path.join(root, 'benchmark.json');
    fs.writeFileSync(
      definitionPath,
      JSON.stringify({
        schema_version: 1,
        skill: 'sample-normalizer',
        runs: 1,
        timeout_ms: 100,
        scenarios: ['train', 'validation'].map((split) => ({
          id: `${split}-timeout`,
          split,
          weight: 1,
          fixture: split,
          executor: { command: process.execPath, args: ['-e', 'setTimeout(() => {}, 1000)'] },
          grader: { command: process.execPath, args: ['-e', 'process.exit(0)'] },
        })),
      }),
    );
    const result = invoke(['--definition', definitionPath, '--skill', SKILL]);
    assert.equal(result.status, 0, result.stderr);
    const report = JSON.parse(result.stdout);
    for (const scenario of report.scenarios) {
      for (const mode of ['with_skill', 'without_skill']) {
        assert.equal(scenario.runs[mode][0].passed, false);
        assert.equal(scenario.runs[mode][0].executor.timed_out, true);
        assert.equal(scenario.runs[mode][0].grader.spawn_error, 'executor did not complete');
      }
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('missing definitions are explicit and non-passing', () => {
  const result = invoke([
    '--definition',
    path.join(os.tmpdir(), 'missing-benchmark.json'),
    '--skill',
    SKILL,
  ]);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /TRUENORTH_BENCHMARK_UNAVAILABLE:/);
});

test('malformed definitions and escaping fixture paths fail closed', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-skill-benchmark-invalid-'));
  try {
    const malformed = path.join(root, 'malformed.json');
    fs.writeFileSync(malformed, JSON.stringify({ schema_version: 1, skill: 'sample', runs: 1 }));
    let result = invoke(['--definition', malformed, '--skill', SKILL]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /scenarios/);

    const escaping = path.join(root, 'escaping.json');
    fs.writeFileSync(
      escaping,
      JSON.stringify({
        schema_version: 1,
        skill: 'sample',
        runs: 1,
        scenarios: [
          {
            id: 'escape',
            split: 'validation',
            weight: 1,
            fixture: '../outside',
            executor: { command: 'node', args: ['executor.js'] },
            grader: { command: 'node', args: ['grader.js'] },
          },
        ],
      }),
    );
    result = invoke(['--definition', escaping, '--skill', SKILL]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /escapes definition directory/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('fixture symlinks are rejected before executor commands run', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-skill-benchmark-symlink-'));
  try {
    const fixture = path.join(root, 'fixture');
    const marker = path.join(root, 'executor-ran');
    fs.mkdirSync(fixture);
    fs.writeFileSync(path.join(root, 'outside.txt'), 'outside');
    fs.symlinkSync(path.join(root, 'outside.txt'), path.join(fixture, 'link.txt'));
    const definitionPath = path.join(root, 'benchmark.json');
    fs.writeFileSync(
      definitionPath,
      JSON.stringify({
        schema_version: 1,
        skill: 'sample-normalizer',
        runs: 1,
        scenarios: ['train', 'validation'].map((split) => ({
          id: `${split}-symlink`,
          split,
          weight: 1,
          fixture: 'fixture',
          executor: {
            command: process.execPath,
            args: ['-e', `require('node:fs').writeFileSync(${JSON.stringify(marker)}, '')`],
          },
          grader: { command: process.execPath, args: ['-e', 'process.exit(0)'] },
        })),
      }),
    );

    const result = invoke(['--definition', definitionPath, '--skill', SKILL]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /fixture contains unsupported symlink/);
    assert.equal(fs.existsSync(marker), false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('baseline regression is authoritative', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-skill-benchmark-regression-'));
  try {
    const baselinePath = path.join(root, 'baseline.json');
    const baseline = JSON.parse(runSample().stdout);
    baseline.validation.delta = 1.01;
    fs.writeFileSync(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`);
    const result = runSample(['--check-baseline', baselinePath]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /validation delta regressed/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
