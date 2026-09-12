// Tests for the npm launcher (task 18.3), using the built-in node:test runner so the
// wrapper needs no test dependencies.
//
// Requirements: 8.3, 8.4, 8.5, 8.9, 8.10.

'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const runner = require('../lib/runner.js');

// A recorder for the injected run dependencies.
function harness(overrides) {
  const state = { exitCode: null, stderr: '', spawned: null };
  const deps = {
    argv: [],
    platform: 'darwin',
    arch: 'arm64',
    cwd: process.cwd(),
    resolver: (request) => `/fake/node_modules/${request}`,
    spawn: (binary, argv, opts) => {
      state.spawned = { binary, argv, opts };
      return { status: 0 };
    },
    stderr: (text) => {
      state.stderr += text;
    },
    exit: (code) => {
      state.exitCode = code;
      return code;
    },
    ...overrides,
  };
  return { deps, state };
}

test('platformPackage builds the scoped name', () => {
  assert.strictEqual(runner.platformPackage('linux', 'x64'), '@truenorth-mcp/linux-x64');
});

test('isSupported covers the four platforms and excludes windows', () => {
  assert.ok(runner.isSupported('darwin', 'arm64'));
  assert.ok(runner.isSupported('darwin', 'x64'));
  assert.ok(runner.isSupported('linux', 'x64'));
  assert.ok(runner.isSupported('linux', 'arm64'));
  assert.ok(!runner.isSupported('win32', 'x64'));
});

test('resolveBinary resolves the platform package binary', () => {
  const resolved = runner.resolveBinary('darwin', 'arm64', (r) => `/n/${r}`);
  assert.strictEqual(resolved, '/n/@truenorth-mcp/darwin-arm64/truenorth-mcp');
});

test('resolveBinary throws for an unsupported platform', () => {
  assert.throws(() => runner.resolveBinary('win32', 'x64', (r) => r));
});

test('run spawns the resolved binary and propagates the exit code', () => {
  const { deps, state } = harness({
    argv: ['--flag'],
    spawn: () => ({ status: 3 }),
  });
  runner.run(deps);
  assert.strictEqual(state.exitCode, 3);
});

test('run exits 1 with a clear message on an unsupported platform', () => {
  const { deps, state } = harness({ platform: 'win32', arch: 'x64' });
  runner.run(deps);
  assert.strictEqual(state.exitCode, 1);
  assert.match(state.stderr, /no prebuilt binary for win32-x64/);
});

test('run exits 1 with a clear message on a spawn failure', () => {
  const { deps, state } = harness({
    spawn: () => ({ error: new Error('ENOENT') }),
  });
  runner.run(deps);
  assert.strictEqual(state.exitCode, 1);
  assert.match(state.stderr, /failed to start the binary/);
});

test('run maps a null spawn status to exit code 1', () => {
  const { deps, state } = harness({ spawn: () => ({ status: null }) });
  runner.run(deps);
  assert.strictEqual(state.exitCode, 1);
});

test('init scaffolds specs when absent', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-init-'));
  const { deps, state } = harness({ argv: ['init'], cwd: dir });
  runner.run(deps);
  assert.strictEqual(state.exitCode, 0);
  assert.ok(fs.existsSync(path.join(dir, 'specs', 'state.yaml')));
  assert.ok(fs.existsSync(path.join(dir, 'specs', 'release-plan.yaml')));
  fs.rmSync(dir, { recursive: true, force: true });
});

test('init skips and reports when specs already exists', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-init-'));
  fs.mkdirSync(path.join(dir, 'specs'));
  fs.writeFileSync(path.join(dir, 'specs', 'state.yaml'), 'active_epic: e01\n');
  const { deps, state } = harness({ argv: ['init'], cwd: dir });
  runner.run(deps);
  assert.strictEqual(state.exitCode, 0);
  assert.match(state.stderr, /scaffolding skipped/);
  // The existing content is left unchanged.
  assert.strictEqual(
    fs.readFileSync(path.join(dir, 'specs', 'state.yaml'), 'utf8'),
    'active_epic: e01\n',
  );
  fs.rmSync(dir, { recursive: true, force: true });
});
