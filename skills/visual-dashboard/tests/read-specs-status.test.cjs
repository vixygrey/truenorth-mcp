'use strict';

const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { afterEach, test } = require('node:test');
const { readSpecsStatus } = require('../scripts/read-specs-status.cjs');

const roots = [];
afterEach(() => {
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function project(profile, releasePlan, state = 'active_task: task-1\nactive_group: null\n') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-dashboard-'));
  roots.push(root);
  fs.mkdirSync(path.join(root, '.agent', 'tasks'), { recursive: true });
  fs.writeFileSync(path.join(root, '.agent', 'profile.yml'), `profile: ${profile}\n`);
  fs.writeFileSync(path.join(root, '.agent', 'tasks', 'state.yml'), state);
  fs.writeFileSync(path.join(root, '.agent', 'tasks', 'release-plan.yml'), releasePlan);
  fs.writeFileSync(
    path.join(root, '.agent', 'tasks', 'execution-status.yml'),
    'tasks: {}\ngroups: {}\ndevelopment_status: {}\n',
  );
  return root;
}

for (const profile of ['issue-per-task', 'kanban', 'generic']) {
  test(`shows ungrouped tasks for ${profile}`, () => {
    const root = project(profile, 'tasks:\n- task_name: Task one\n  verify_command: true\n');
    const status = readSpecsStatus(root);
    assert.strictEqual(status.profile, profile);
    assert.deepStrictEqual(
      status.tasks.map((task) => task.title),
      ['Task one'],
    );
    assert.deepStrictEqual(status.groups, []);
    assert.strictEqual(status.active_group_id, null);
  });
}

for (const [profile, kind] of [
  ['epic-based', 'epic'],
  ['milestone-based', 'milestone'],
]) {
  test(`shows profile-derived groups for ${profile}`, () => {
    const root = project(
      profile,
      `groups:\n- group_id: g1\n  group_kind: ${kind}\n  title: Group one\n  capsule_dir: .agent/tasks/g1\ntasks:\n- group_id: g1\n  group_kind: ${kind}\n  task_name: Task one\n  verify_command: true\n`,
      'active_task: Task one\nactive_group: g1\n',
    );
    const status = readSpecsStatus(root);
    assert.deepStrictEqual(
      status.groups.map((group) => group.kind),
      [kind],
    );
    assert.deepStrictEqual(
      status.tasks.map((task) => task.group_id),
      ['g1'],
    );
    assert.strictEqual(status.active_group_id, 'g1');
  });
}

test('reads legacy epic metadata without emitting it as the current schema', () => {
  const root = project(
    'issue-per-task',
    'epics:\n- id: e01\n  title: Legacy epic\n  wsjf: 2.5\n  file: e01/group.yml\n',
    'active_epic: e01\n',
  );
  const status = readSpecsStatus(root);
  assert.deepStrictEqual(
    status.groups.map((group) => group.title),
    ['Legacy epic'],
  );
  assert.strictEqual(status.groups[0].legacy, true);
});
