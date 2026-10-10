'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync, spawnSync } = require('node:child_process');
const test = require('node:test');
const { METRIC, hash, sourceSnapshot } = require('./coverage-report.js');
const cli = path.join(__dirname, 'coverage.js');

function fixture(t, businessCovered = 95, wrapperCovered = 65) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-coverage-test-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  function write(file, text) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, text);
  }
  for (const dir of ['runtime/tests', 'npm/test', 'scripts', '.github/workflows'])
    fs.mkdirSync(path.join(root, dir), { recursive: true });
  for (const file of [
    'runtime/Cargo.toml',
    'runtime/Cargo.lock',
    'rust-toolchain.toml',
    '.node-version',
    'package.json',
    'package-lock.json',
    'npm/package.json',
  ])
    write(file, 'fixture\n');
  write(
    '.agent/config/rules.yml',
    'advisory:\n  quality_gates:\n    coverage:\n      overall_minimum_percent: 80\n      business_logic_minimum_percent: 95\n',
  );
  write('runtime/src/config.rs', 'fn production() {}\n'.repeat(100));
  write('npm/lib/runner.js', 'production();\n'.repeat(99));
  write('npm/bin/truenorth.js', 'production();\n');
  const env = { PATH: process.env.PATH };
  execFileSync('git', ['init', '--quiet', root], { env });
  execFileSync(
    'git',
    [
      '-c',
      'user.name=Coverage Test',
      '-c',
      'user.email=coverage@example.invalid',
      '-c',
      'core.hooksPath=/dev/null',
      'commit',
      '--quiet',
      '--allow-empty',
      '-m',
      'fixture',
    ],
    { cwd: root, env },
  );
  const report = path.join(root, 'coverage');
  fs.mkdirSync(report);
  function lcov(name, count, covered) {
    return (
      `SF:${name}\n` +
      Array.from({ length: count }, (_, i) => `DA:${i + 1},${i < covered ? 1 : 0}\n`).join('') +
      `LF:${count}\nLH:${covered}\nend_of_record\n`
    );
  }
  const raw = {
    rust: lcov('runtime/src/config.rs', 100, businessCovered),
    node: lcov('npm/lib/runner.js', 99, wrapperCovered - 1) + lcov('npm/bin/truenorth.js', 1, 1),
  };
  const metadata = {
    schema_version: 1,
    metric: METRIC,
    build_modes: ['default', 'tree-sitter'],
    ...sourceSnapshot(root),
    generated_at: new Date().toISOString(),
    repository_root: root,
    tools: { cargo_llvm_cov: 'cargo-llvm-cov 0.9.1', rust: 'rustc 1.88.0', node: process.version },
    raw_sha256: {},
  };
  function save() {
    for (const layer of ['rust', 'node']) {
      fs.writeFileSync(path.join(report, `${layer}.lcov`), raw[layer]);
      metadata.raw_sha256[layer] = hash(raw[layer]);
    }
    fs.writeFileSync(path.join(report, 'report.json'), JSON.stringify(metadata));
  }
  save();
  return { root, report, raw, metadata, save, write };
}

function check(f) {
  return spawnSync(process.execPath, [cli, 'check', '--root', f.root, '--report', f.report], {
    encoding: 'utf8',
  });
}
function rejected(result, explanation) {
  assert.equal(result.status, 1, result.stdout + result.stderr);
  assert.match(result.stderr, explanation);
}

test('exact 80 overall and 95 business thresholds pass without rounding', (t) => {
  const f = fixture(t);
  const result = check(f);
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.overall.covered, 160);
  assert.equal(report.overall.mapped, 200);
  assert.equal(report.business_logic.percent, 95);
  assert.equal(report.passed, true);
});

test('each threshold rejects insufficient coverage independently', (t) => {
  rejected(check(fixture(t, 95, 64)), /overall.*79\.50.*80/);
  rejected(check(fixture(t, 94, 67)), /business_logic.*94\.00.*95/);
});

test('missing report metadata and either raw layer fail closed', (t) => {
  for (const filename of ['report.json', 'rust.lcov', 'node.lcov']) {
    const f = fixture(t);
    fs.unlinkSync(path.join(f.report, filename));
    rejected(check(f), new RegExp(filename.replace('.', '\\.')));
  }
});

test('malformed metadata, unsupported schema and wrong build matrix are rejected', (t) => {
  const f = fixture(t);
  fs.writeFileSync(path.join(f.report, 'report.json'), '{');
  rejected(check(f), /report\.json/);
  f.metadata.schema_version = 2;
  f.save();
  rejected(check(f), /schema/);
  f.metadata.schema_version = 1;
  f.metadata.build_modes = ['default'];
  f.save();
  rejected(check(f), /build_modes/);
});

test('changed source and wrong head cannot reuse a report', (t) => {
  const changed = fixture(t);
  changed.write('runtime/src/config.rs', 'fn changed() {}\n'.repeat(100));
  rejected(check(changed), /source_sha256.*stale/);
  const wrongHead = fixture(t);
  wrongHead.metadata.head = '0'.repeat(40);
  wrongHead.save();
  rejected(check(wrongHead), /head.*stale/);
});

test('expired and future measurement timestamps are rejected', (t) => {
  for (const offset of [-86400001, 120000]) {
    const f = fixture(t);
    f.metadata.generated_at = new Date(Date.now() + offset).toISOString();
    f.save();
    rejected(check(f), /generated_at/);
  }
});

test('raw report changes and malformed native line summaries are rejected', (t) => {
  const changed = fixture(t);
  fs.appendFileSync(path.join(changed.report, 'rust.lcov'), 'garbage\n');
  rejected(check(changed), /rust\.lcov.*raw_sha256/);
  for (const transform of [
    (text) => text.replace('LH:95', 'LH:101'),
    (text) => text.replace('DA:1,1', 'DA:1,NaN'),
    (text) => text.replace('DA:2,1', 'DA:1,1'),
    (text) => text.replace('end_of_record\n', ''),
    (text) => text.replace('SF:runtime/src/config.rs', 'SF:../outside.rs'),
    (text) => text.replace('DA:1,1', 'FNDA:NaN,production\nDA:1,1'),
    (text) => text.replace('DA:1,1', 'BRDA:1,0,0,NaN\nDA:1,1'),
    (text) => text.replace('DA:1,1', 'FNF:-1\nDA:1,1'),
    (text) => 'FNDA:1,production\n' + text,
  ]) {
    const f = fixture(t);
    f.raw.rust = transform(f.raw.rust);
    f.save();
    rejected(check(f), /rust\.lcov.*malformed/);
  }
});

test('an unloaded shipped wrapper entrypoint is counted as uncovered', (t) => {
  const f = fixture(t, 95, 65);
  f.raw.node = f.raw.node.slice(0, f.raw.node.indexOf('SF:npm/bin/truenorth.js'));
  f.save();
  rejected(check(f), /overall.*79\.50.*80/);
});

test('out-of-range mapped lines and an empty business source set are rejected', (t) => {
  const outOfRange = fixture(t);
  outOfRange.raw.rust = outOfRange.raw.rust.replace('DA:100,0', 'DA:101,0');
  outOfRange.save();
  rejected(check(outOfRange), /line 101.*100/);
  const empty = fixture(t);
  empty.raw.rust = empty.raw.rust.replace('runtime/src/config.rs', 'runtime/src/config_tests.rs');
  empty.save();
  rejected(check(empty), /no mapped production lines/);
});

test('coverage policy cannot silently lower an approved threshold', (t) => {
  const f = fixture(t);
  f.write(
    '.agent/config/rules.yml',
    'advisory:\n  quality_gates:\n    coverage:\n      overall_minimum_percent: 79\n      business_logic_minimum_percent: 95\n',
  );
  f.metadata.source_sha256 = sourceSnapshot(f.root).source_sha256;
  f.save();
  rejected(check(f), /overall_minimum_percent.*80/);
});

test('coverage thresholds must belong to the advisory quality-gate mapping', (t) => {
  const f = fixture(t);
  f.write(
    '.agent/config/rules.yml',
    'advisory:\n  quality_gates: {}\nunrelated:\n    coverage:\n      overall_minimum_percent: 80\n      business_logic_minimum_percent: 95\n',
  );
  f.metadata.source_sha256 = sourceSnapshot(f.root).source_sha256;
  f.save();
  rejected(check(f), /coverage policy.*quality_gates/);
});

test('LLVM summary differences cannot replace the declared unique DA line counts', (t) => {
  const f = fixture(t);
  f.raw.rust = f.raw.rust.replace('LF:100', 'LF:102').replace('LH:95', 'LH:97');
  f.save();
  const result = check(f);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).business_logic.percent, 95);
  f.raw.node = f.raw.node.replace('LH:64', 'LH:65');
  f.save();
  rejected(check(f), /node\.lcov.*line summary/);
});

test('ambient Git selection cannot change coverage HEAD provenance', (t) => {
  const measured = fixture(t);
  const unrelated = fixture(t);
  function commit(root, message) {
    execFileSync(
      'git',
      [
        '-c',
        'user.name=Coverage Test',
        '-c',
        'user.email=coverage@example.invalid',
        '-c',
        'core.hooksPath=/dev/null',
        'commit',
        '--quiet',
        '--allow-empty',
        '-m',
        message,
      ],
      { cwd: root, env: { PATH: process.env.PATH } },
    );
  }
  commit(unrelated.root, 'unrelated-head');
  const env = {
    ...process.env,
    GIT_DIR: path.join(unrelated.root, '.git'),
    GIT_WORK_TREE: unrelated.root,
  };
  const args = [cli, 'check', '--root', measured.root, '--report', measured.report];
  const accepted = spawnSync(process.execPath, args, { encoding: 'utf8', env });
  assert.equal(accepted.status, 0, accepted.stderr);
  assert.equal(JSON.parse(accepted.stdout).head, measured.metadata.head);
  commit(measured.root, 'measured-head-changed');
  rejected(spawnSync(process.execPath, args, { encoding: 'utf8', env }), /head.*stale/);
});
