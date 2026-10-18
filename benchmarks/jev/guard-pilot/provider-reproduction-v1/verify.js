'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const load = (name) => JSON.parse(fs.readFileSync(path.join(__dirname, name), 'utf8'));
const before = load('request-before.json');
const after = load('request-after.json');
const fixture = load('case.json');
assert.deepEqual(before.state, after.state);
assert.equal(after.model, 'jev-latest');
assert.equal(before.model, after.model);
assert.deepEqual(after.state.proposal.paths, fixture.proposal.paths);
assert.equal(after.state.proposal.content, fixture.proposal.content.trimEnd());
for (const key of ['complexity', 'contains_secrets', 'violates_conventions']) {
  assert.deepEqual(before.questions[key], after.questions[key]);
}
assert.notDeepEqual(before.questions.hallucinated_import, after.questions.hallucinated_import);
for (const name of ['pilot-before.json', 'pilot-after.json']) {
  const report = load(name);
  assert.equal(report.accepted, false);
  assert.equal(report.policy_approval, 'pending');
  assert.equal(report.cases.length, 1);
  assert.equal(
    report.cases[0].case_id_sha256,
    require('node:crypto').createHash('sha256').update(fixture.case_id).digest('hex'),
  );
  assert.equal(report.cases[0].observed_decision, 'allow');
  assert.equal(report.cases[0].expected_decision, 'annotate');
}
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'typesafe-import-repro-'));
const root = path.join(temporary, 'project');
try {
  fs.mkdirSync(root);
  for (const file of fixture.files) {
    assert.ok(['Cargo.toml', 'src/check.rs'].includes(file.path));
    const target = path.join(root, file.path);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, file.content);
  }
  const env = Object.fromEntries(
    ['PATH', 'RUSTUP_TOOLCHAIN']
      .filter((key) => process.env[key] !== undefined)
      .map((key) => [key, process.env[key]]),
  );
  Object.assign(env, {
    HOME: temporary,
    RUSTUP_HOME: process.env.RUSTUP_HOME || path.join(os.homedir(), '.rustup'),
    CARGO_HOME: path.join(temporary, 'cargo-home'),
    CARGO_TARGET_DIR: path.join(temporary, 'target'),
  });
  const version = spawnSync('rustc', ['--version'], { env, encoding: 'utf8' });
  assert.equal(version.status, 0, version.error?.message || version.stderr);
  const results = [];
  for (const stage of ['baseline', 'proposal']) {
    if (stage === 'proposal')
      fs.writeFileSync(path.join(root, 'src/check.rs'), fixture.proposal.content);
    const run = spawnSync('cargo', ['check', '--offline', '--lib', '--message-format=json'], {
      cwd: root,
      env,
      encoding: 'utf8',
      timeout: 30000,
      maxBuffer: 1048576,
    });
    assert.ifError(run.error);
    const messages = run.stdout
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line));
    const errors = messages.filter(
      (item) => item.reason === 'compiler-message' && item.message.level === 'error',
    );
    if (stage === 'baseline') {
      assert.equal(run.status, 0, run.stderr);
      assert.equal(errors.length, 0);
    } else {
      assert.equal(run.status, 101, run.stderr);
      assert.ok(errors.length > 0);
      assert.ok(
        errors.every(
          (item) =>
            item.message.code?.code === 'E0432' &&
            item.message.spans.some(
              (span) =>
                span.is_primary &&
                span.file_name === 'src/check.rs' &&
                span.text.some((line) => line.text.includes('absent_codec')),
            ),
        ),
      );
    }
    results.push({ stage, exit_code: run.status, diagnostics: errors.map((item) => item.message) });
  }
  console.log(
    JSON.stringify(
      { rustc: version.stdout.trim(), provider_calls: 0, baseline_and_proposal: results },
      null,
      2,
    ),
  );
} finally {
  fs.rmSync(temporary, { recursive: true, force: true });
}
