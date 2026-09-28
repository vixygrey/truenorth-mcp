'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const UNAVAILABLE_MARKER = 'TRUENORTH_VERIFY_UNAVAILABLE:';

function scalar(frontmatter, key) {
  const match = frontmatter.match(new RegExp(`^${key}:\\s*(.+?)\\s*$`, 'm'));
  if (!match) return null;
  const value = match[1].trim();
  if (
    value.length >= 2 &&
    ((value.startsWith("'") && value.endsWith("'")) ||
      (value.startsWith('"') && value.endsWith('"')))
  ) {
    return value.slice(1, -1);
  }
  return value;
}

function readMetadata(file) {
  const source = fs.readFileSync(file, 'utf8');
  const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
  if (!match) return { kind: null, verify: null };
  return {
    kind: scalar(match[1], 'kind'),
    verify: scalar(match[1], 'verify'),
  };
}

function discover(root) {
  const skillsRoot = path.join(root, 'skills');
  return fs
    .readdirSync(skillsRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => ({
      name: entry.name,
      file: path.join(skillsRoot, entry.name, 'SKILL.md'),
    }))
    .filter((entry) => fs.existsSync(entry.file))
    .map((entry) => ({ ...entry, ...readMetadata(entry.file) }))
    .filter((entry) => entry.kind === 'scripted')
    .sort((left, right) => left.name.localeCompare(right.name));
}

function run(root, output = process.stdout, errorOutput = process.stderr) {
  const skills = discover(root);
  let failed = 0;
  let unavailable = 0;

  for (const skill of skills) {
    if (!skill.verify) {
      errorOutput.write(`verify-skills: FAIL ${skill.name} (missing frontmatter.verify)\n`);
      failed += 1;
      continue;
    }

    const result = spawnSync(skill.verify, {
      cwd: root,
      encoding: 'utf8',
      env: process.env,
      shell: true,
      timeout: 120_000,
    });
    const combined = `${result.stdout || ''}${result.stderr || ''}`;

    if (combined.includes(UNAVAILABLE_MARKER)) {
      const detail = combined
        .split(/\r?\n/)
        .find((line) => line.includes(UNAVAILABLE_MARKER))
        .split(UNAVAILABLE_MARKER)[1]
        .trim();
      errorOutput.write(`verify-skills: UNAVAILABLE ${skill.name} (${detail})\n`);
      unavailable += 1;
    } else if (result.error || result.status !== 0) {
      const detail = result.error ? result.error.message : `exit ${result.status}`;
      errorOutput.write(`verify-skills: FAIL ${skill.name} (${detail})\n`);
      if (combined.trim()) errorOutput.write(combined);
      failed += 1;
    } else {
      output.write(`verify-skills: PASS ${skill.name}\n`);
    }
  }

  output.write(
    `verify-skills: ${skills.length - failed - unavailable} passed, ${failed} failed, ${unavailable} unavailable.\n`,
  );
  return failed === 0 && unavailable === 0 ? 0 : 1;
}

if (require.main === module) {
  process.exitCode = run(path.resolve(__dirname, '..'));
}

module.exports = { UNAVAILABLE_MARKER, discover, readMetadata, run };
