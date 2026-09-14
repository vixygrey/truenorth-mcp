'use strict';

const fs = require('fs');
const path = require('path');

function readFileSafe(p) {
  try {
    return fs.readFileSync(p, 'utf-8');
  } catch {
    return null;
  }
}

function parseTopLevelScalars(text) {
  const out = {};
  for (const line of text.split(/\r?\n/)) {
    const m = line.match(/^([a-zA-Z0-9_]+):\s*(.+)$/);
    if (m) out[m[1]] = m[2].replace(/^["']|["']$/g, '');
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
    if (/^\S/.test(line) && !line.startsWith(' ')) break;
    const m = line.match(/^\s+([a-zA-Z0-9_]+):\s*(.+)$/);
    if (m) out[m[1]] = m[2].replace(/^["']|["']$/g, '');
  }
  return out;
}

function parseEpicsFromReleasePlan(text) {
  const epics = [];
  const blocks = text.split(/\n\s*-\s+id:\s+/).slice(1);
  for (const block of blocks) {
    const id = block.match(/^(\S+)/)?.[1];
    const title = block.match(/(?:title|name):\s*"?([^"\n]+)"?/)?.[1];
    const wsjf = parseFloat(block.match(/wsjf:\s*([\d.]+)/)?.[1] || '0');
    const file = block.match(/file:\s*(\S+)/)?.[1];
    if (id) epics.push({ id, title: title || id, wsjf, file });
  }
  return epics;
}

function parseSimpleEpic(text) {
  const title = text.match(/^(?:title|name):\s*"?([^"\n]+)"?/m)?.[1];
  const stories = [];
  const storyBlocks = text.split(/\n\s*-\s+id:\s+/).slice(1);
  for (const sb of storyBlocks) {
    const sid = sb.match(/^(\S+)/)?.[1];
    const stitle = sb.match(/title:\s*"?([^"\n]+)"?/)?.[1];
    if (sid) stories.push({ id: sid, title: stitle || sid });
  }
  return { title, stories };
}

function readSpecsStatus(projectDir) {
  // The cockpit lives under .agent/tasks/ after the relocation.
  const tasksDir = path.join(projectDir, '.agent', 'tasks');
  const stateText = readFileSafe(path.join(tasksDir, 'state.yml')) || '';
  const releaseText = readFileSafe(path.join(tasksDir, 'release-plan.yml')) || '';
  const execText = readFileSafe(path.join(tasksDir, 'execution-status.yml')) || '';

  const stateScalars = parseTopLevelScalars(stateText);
  const state = {
    active_flow: stateScalars.active_flow,
    // Read the neutral grouping key, falling back to the legacy epic keys.
    active_group_id:
      stateScalars.active_group_id || stateScalars.active_group || stateScalars.active_epic,
    git: parseNestedBlock(stateText, 'git'),
    handoff: parseNestedBlock(stateText, 'handoff'),
    group_cycle:
      parseNestedBlock(stateText, 'group_cycle') || parseNestedBlock(stateText, 'epic_cycle'),
  };

  const release = parseNestedBlock(releaseText, 'release');
  const devStatus = parseNestedBlock(execText, 'development_status');
  const groups = parseEpicsFromReleasePlan(releaseText);

  const activeGroupId = state.active_group_id;
  let activeGroup = null;
  const groupMeta = groups.find((g) => g.id === activeGroupId);
  if (groupMeta && groupMeta.file) {
    const groupText = readFileSafe(path.join(tasksDir, groupMeta.file));
    if (groupText) activeGroup = parseSimpleEpic(groupText);
  }

  // Planning status is no longer tracked in an in-repo file, so it is empty.
  const planning = {};
  if (false) {
    const planningText = '';
    const wfBlocks = planningText.split(/\n\s{2}([a-z-]+):/);
    for (let i = 1; i < wfBlocks.length; i += 2) {
      const key = wfBlocks[i];
      const block = wfBlocks[i + 1] || '';
      const status = block.match(/status:\s*(\S+)/)?.[1];
      planning[key] = { status };
    }
  }

  return {
    projectDir,
    state,
    release,
    groups: groups.map((g) => ({
      ...g,
      status: devStatus[g.id] || 'pending',
    })),
    execution_status: devStatus,
    planning_status: planning,
    active_group: activeGroup,
    active_group_id: activeGroupId,
  };
}

module.exports = { readSpecsStatus };

if (require.main === module) {
  const dir = process.argv[2] || process.cwd();
  console.log(JSON.stringify(readSpecsStatus(path.resolve(dir)), null, 2));
}
