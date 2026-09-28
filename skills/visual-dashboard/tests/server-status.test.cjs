'use strict';

const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const readline = require('node:readline');
const { test } = require('node:test');

const ROOT = path.resolve(__dirname, '..', '..', '..');
const SERVER = path.join(ROOT, 'skills', 'visual-dashboard', 'scripts', 'server.cjs');

function writeFixture(root) {
  fs.mkdirSync(path.join(root, '.agent', 'tasks'), { recursive: true });
  fs.writeFileSync(path.join(root, '.agent', 'profile.yml'), 'profile: epic-based\n');
  fs.writeFileSync(
    path.join(root, '.agent', 'tasks', 'state.yml'),
    'phase: execute\nactive_task: task-1\nactive_group: epic-1\n',
  );
  fs.writeFileSync(
    path.join(root, '.agent', 'tasks', 'release-plan.yml'),
    'groups:\n- group_id: epic-1\n  group_kind: epic\n  title: Epic one\n' +
      'tasks:\n- group_id: epic-1\n  group_kind: epic\n  task_name: Task one\n' +
      '  verify_command: true\n',
  );
  fs.writeFileSync(
    path.join(root, '.agent', 'tasks', 'execution-status.yml'),
    'tasks:\n  task-1: in-progress\ngroups:\n  epic-1: in-progress\n',
  );
}

function waitForStart(child) {
  return new Promise((resolve, reject) => {
    const lines = readline.createInterface({ input: child.stdout });
    const timer = setTimeout(
      () => reject(new Error('dashboard did not start within 5 seconds')),
      5000,
    );
    timer.unref();

    child.once('error', reject);
    child.once('exit', (code, signal) => {
      reject(new Error(`dashboard exited before startup: ${code ?? signal}`));
    });
    lines.on('line', (line) => {
      let message;
      try {
        message = JSON.parse(line);
      } catch {
        return;
      }
      if (message.type === 'server-started') {
        clearTimeout(timer);
        lines.close();
        resolve(message);
      }
    });
  });
}

function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((resolve) => {
    child.once('exit', resolve);
    child.kill('SIGTERM');
  });
}

test('serves fixture cockpit data from /api/status', async () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-dashboard-http-'));
  const project = path.join(temp, 'project');
  const dashboard = path.join(temp, 'dashboard');
  writeFixture(project);

  const child = spawn(process.execPath, [SERVER], {
    cwd: ROOT,
    env: {
      ...process.env,
      TRUENORTH_DASHBOARD_DIR: dashboard,
      TRUENORTH_DASHBOARD_HOST: '127.0.0.1',
      TRUENORTH_DASHBOARD_PORT: '0',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stderr = '';
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });

  try {
    const started = await waitForStart(child);
    assert(
      Number.isInteger(started.port) && started.port > 0,
      `invalid bound port: ${started.port}`,
    );
    const url = new URL('/api/status', started.url);
    url.searchParams.set('projectDir', project);
    const response = await fetch(url);
    assert.strictEqual(response.status, 200, stderr);
    const status = await response.json();
    assert.strictEqual(status.profile, 'epic-based');
    assert.deepStrictEqual(
      status.tasks.map((task) => task.title),
      ['Task one'],
    );
    assert.deepStrictEqual(
      status.groups.map((group) => group.title),
      ['Epic one'],
    );
    assert.strictEqual(status.active_group_id, 'epic-1');
  } finally {
    await stop(child);
    fs.rmSync(temp, { recursive: true, force: true });
  }
});
