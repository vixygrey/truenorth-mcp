'use strict';

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

function createPackedWrapperFixture(platformPackage, prefix = 'tn-packed-mcp-') {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  try {
    const packs = path.join(work, 'packs');
    const install = path.join(work, 'install');
    const project = path.join(work, 'project');
    fs.mkdirSync(packs);
    fs.mkdirSync(install);
    fs.mkdirSync(project);
    fs.writeFileSync(path.join(install, 'package.json'), '{"private":true}\n');

    const platformTarball = pack(platformPackage, packs);
    const wrapperTarball = pack('npm', packs);
    execFileSync(
      'npm',
      [
        'install',
        '--ignore-scripts',
        '--no-audit',
        '--no-fund',
        '--offline',
        platformTarball,
        wrapperTarball,
      ],
      { cwd: install, stdio: 'inherit' },
    );

    const wrapper = path.join(install, 'node_modules', 'truenorth-mcp', 'bin', 'truenorth.js');
    runWrapper(wrapper, ['init', '--profile', 'generic'], project);
    runWrapper(wrapper, ['--check-config'], project);

    return {
      work,
      project,
      command: process.execPath,
      args: [wrapper],
      cleanup() {
        fs.rmSync(work, { recursive: true, force: true });
      },
    };
  } catch (error) {
    fs.rmSync(work, { recursive: true, force: true });
    throw error;
  }
}

function pack(packageDirectory, destination) {
  const output = execFileSync(
    'npm',
    ['pack', '--json', '--pack-destination', destination, path.resolve(packageDirectory)],
    { encoding: 'utf8' },
  );
  const [{ filename }] = JSON.parse(output);
  return path.join(destination, filename);
}

function runWrapper(wrapper, args, root) {
  execFileSync(process.execPath, [wrapper, ...args], {
    cwd: root,
    env: { ...process.env, TRUENORTH_ROOT: root, TRUENORTH_VERIFY_CMD: 'true' },
    stdio: 'inherit',
  });
}

module.exports = { createPackedWrapperFixture };
