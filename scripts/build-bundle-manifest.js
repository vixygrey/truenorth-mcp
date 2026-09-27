'use strict';

const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

function trackedSkillPaths(repoRoot) {
  const output = execFileSync('git', ['ls-files', '-z', 'npm/skills'], {
    cwd: repoRoot,
    encoding: 'buffer',
  });
  return output
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .sort()
    .map((entry) => entry.replace(/^npm\//, ''));
}

function fileRecord(packageRoot, relative) {
  const absolute = path.join(packageRoot, relative);
  const bytes = fs.readFileSync(absolute);
  const executable = (fs.statSync(absolute).mode & 0o111) !== 0;
  return {
    path: relative.split(path.sep).join('/'),
    sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    mode: executable ? '0755' : '0644',
  };
}

function buildManifest(repoRoot) {
  const packageRoot = path.join(repoRoot, 'npm');
  const packageJson = JSON.parse(fs.readFileSync(path.join(packageRoot, 'package.json'), 'utf8'));
  return {
    schema_version: 1,
    bundle_version: packageJson.version,
    workspace_schema_version: '1',
    supported_from: ['1.0.2'],
    files: trackedSkillPaths(repoRoot).map((relative) => fileRecord(packageRoot, relative)),
  };
}

function serialized(manifest) {
  const json = JSON.stringify(manifest, null, 2).replace(
    '\"supported_from\": [\n    \"1.0.2\"\n  ]',
    '\"supported_from\": [\"1.0.2\"]',
  );
  return `${json}\n`;
}

function main(argv) {
  const repoRoot = path.resolve(__dirname, '..');
  const target = path.join(repoRoot, 'npm', 'bundle', 'current.json');
  const expected = serialized(buildManifest(repoRoot));
  if (argv.includes('--check')) {
    const actual = fs.readFileSync(target, 'utf8');
    if (actual !== expected) {
      throw new Error(
        'npm/bundle/current.json is stale; run node scripts/build-bundle-manifest.js',
      );
    }
    return;
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, expected);
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`bundle manifest: ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { buildManifest, fileRecord, serialized };
