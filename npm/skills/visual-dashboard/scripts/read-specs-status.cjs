'use strict';

const fs = require('fs');
const path = require('path');

function readFileSafe(file) {
  try {
    return fs.readFileSync(file, 'utf8');
  } catch {
    return null;
  }
}

function scalar(value) {
  const clean = String(value)
    .trim()
    .replace(/^["']|["']$/g, '');
  if (clean === 'null' || clean === '~') return null;
  return clean;
}

function parseTopLevelScalars(text) {
  const out = {};
  for (const line of text.split(/\r?\n/)) {
    const match = line.match(/^([a-zA-Z0-9_]+):\s*(.+)$/);
    if (match) out[match[1]] = scalar(match[2]);
  }
  return out;
}

function parseNestedBlock(text, parentKey) {
  const out = {};
  let inBlock = false;
  for (const line of text.split(/\r?\n/)) {
    if (line.match(new RegExp(`^${parentKey}:`))) {
      inBlock = true;
      continue;
    }
    if (!inBlock) continue;
    if (/^\S/.test(line)) break;
    const match = line.match(/^\s+([a-zA-Z0-9_]+):\s*(.+)$/);
    if (match) out[match[1]] = scalar(match[2]);
  }
  return out;
}

function parseSequence(text, key) {
  const records = [];
  let inSequence = false;
  let record = null;
  for (const line of text.split(/\r?\n/)) {
    if (line === `${key}:`) {
      inSequence = true;
      continue;
    }
    if (!inSequence) continue;
    if (/^[a-zA-Z0-9_]+:/.test(line)) break;
    const first = line.match(/^\s*-\s+([a-zA-Z0-9_]+):\s*(.*)$/);
    if (first) {
      record = { [first[1]]: scalar(first[2]) };
      records.push(record);
      continue;
    }
    const field = line.match(/^\s+([a-zA-Z0-9_]+):\s*(.*)$/);
    if (record && field) record[field[1]] = scalar(field[2]);
  }
  return records;
}

function normalizeTask(task, index) {
  return {
    id: task.task_id || task.id || task.task_name || `task-${index + 1}`,
    title: task.task_name || task.title || task.task_id || task.id || `Task ${index + 1}`,
    verify_command: task.verify_command || task.verify || null,
    group_id: task.group_id || null,
    group_kind: task.group_kind || null,
  };
}

function normalizeGroup(group, legacy = false) {
  return {
    id: group.group_id || group.id,
    kind: group.group_kind || (legacy ? 'epic' : null),
    title: group.title || group.name || group.group_id || group.id,
    file: group.file || group.capsule_dir || null,
    priority: group.priority || group.wsjf || null,
    legacy,
  };
}

function readSpecsStatus(projectDir) {
  const agentDir = path.join(projectDir, '.agent');
  const tasksDir = path.join(agentDir, 'tasks');
  const profileText = readFileSafe(path.join(agentDir, 'profile.yml')) || '';
  const stateText = readFileSafe(path.join(tasksDir, 'state.yml')) || '';
  const releaseText = readFileSafe(path.join(tasksDir, 'release-plan.yml')) || '';
  const executionText = readFileSafe(path.join(tasksDir, 'execution-status.yml')) || '';

  const profile = parseTopLevelScalars(profileText).profile || 'issue-per-task';
  const stateScalars = parseTopLevelScalars(stateText);
  const activeGroupId =
    stateScalars.active_group || stateScalars.active_group_id || stateScalars.active_epic || null;
  const state = {
    active_flow: stateScalars.active_flow || null,
    active_task: stateScalars.active_task || stateScalars.active_story || null,
    active_group: activeGroupId,
    git: parseNestedBlock(stateText, 'git'),
    handoff: parseNestedBlock(stateText, 'handoff'),
    group_cycle: parseNestedBlock(stateText, 'group_cycle'),
  };

  const tasks = parseSequence(releaseText, 'tasks').map(normalizeTask);
  const currentGroups = parseSequence(releaseText, 'groups').map((group) =>
    normalizeGroup(group, false),
  );
  const legacyGroups = parseSequence(releaseText, 'epics').map((group) =>
    normalizeGroup(group, true),
  );
  const groups = currentGroups.length > 0 ? currentGroups : legacyGroups;
  const developmentStatus = parseNestedBlock(executionText, 'development_status');
  const taskStatus = parseNestedBlock(executionText, 'tasks');
  const groupStatus = parseNestedBlock(executionText, 'groups');

  return {
    projectDir,
    profile,
    state,
    release: parseNestedBlock(releaseText, 'release'),
    tasks: tasks.map((task) => ({
      ...task,
      status: taskStatus[task.id] || developmentStatus[task.id] || 'pending',
    })),
    groups: groups.map((group) => ({
      ...group,
      status: groupStatus[group.id] || developmentStatus[group.id] || 'pending',
    })),
    execution_status: developmentStatus,
    planning_status: {},
    active_group: groups.find((group) => group.id === activeGroupId) || null,
    active_group_id: activeGroupId,
  };
}

module.exports = { readSpecsStatus };

if (require.main === module) {
  const dir = process.argv[2] || process.cwd();
  console.log(JSON.stringify(readSpecsStatus(path.resolve(dir)), null, 2));
}
