'use strict';

const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

function trackedPaths(repoRoot, prefix) {
  const output = execFileSync('git', ['ls-files', '-z', prefix], {
    cwd: repoRoot,
    encoding: 'buffer',
  });
  return output.toString('utf8').split('\0').filter(Boolean).sort();
}

function trackedSkillPaths(repoRoot) {
  return trackedPaths(repoRoot, 'npm/skills').map((entry) => entry.replace(/^npm\//, ''));
}

function parseCatalog(repoRoot) {
  const source = fs.readFileSync(path.join(repoRoot, 'skills', 'catalog.yml'), 'utf8');
  const catalog = { schema_version: null, sets: {}, skills: {} };
  let section = null;
  let currentSet = null;
  for (const [index, line] of source.split('\n').entries()) {
    if (!line.trim()) continue;
    let match = line.match(/^schema_version: (\d+)$/);
    if (match) {
      catalog.schema_version = Number(match[1]);
      continue;
    }
    match = line.match(/^(sets|skills):$/);
    if (match) {
      section = match[1];
      currentSet = null;
      continue;
    }
    if (section === 'sets' && (match = line.match(/^  ([a-z][a-z0-9-]*):$/))) {
      currentSet = match[1];
      catalog.sets[currentSet] = {};
      continue;
    }
    if (
      section === 'sets' &&
      currentSet &&
      (match = line.match(/^    (support|description|prerequisites): (.+)$/))
    ) {
      catalog.sets[currentSet][match[1]] = match[2];
      continue;
    }
    if (section === 'skills' && (match = line.match(/^  ([a-z][a-z0-9-]*): ([a-z][a-z0-9-]*)$/))) {
      catalog.skills[match[1]] = match[2];
      continue;
    }
    throw new Error(`skills/catalog.yml:${index + 1}: unsupported catalog syntax`);
  }
  validateCatalog(repoRoot, catalog);
  return catalog;
}

function validateCatalog(repoRoot, catalog) {
  if (catalog.schema_version !== 1)
    throw new Error('skills/catalog.yml: unsupported schema version');
  if (!catalog.sets.core) throw new Error('skills/catalog.yml: missing core set');
  for (const [name, metadata] of Object.entries(catalog.sets)) {
    for (const field of ['support', 'description', 'prerequisites']) {
      if (!metadata[field]) throw new Error(`skills/catalog.yml: set ${name} is missing ${field}`);
    }
  }
  const rootFiles = trackedPaths(repoRoot, 'skills');
  const rootSkills = rootFiles
    .filter((entry) => /^skills\/[^/]+\/SKILL\.md$/.test(entry))
    .map((entry) => entry.split('/')[1]);
  const declared = Object.keys(catalog.skills).sort();
  if (JSON.stringify(rootSkills) !== JSON.stringify(declared)) {
    throw new Error(
      'skills/catalog.yml: declared skills do not exactly match tracked skill directories',
    );
  }
  for (const [skill, set] of Object.entries(catalog.skills)) {
    if (!catalog.sets[set])
      throw new Error(`skills/catalog.yml: ${skill} names unknown set ${set}`);
  }
  validateMirror(
    repoRoot,
    rootFiles.filter((entry) => entry !== 'skills/catalog.yml'),
  );
}

function validateMirror(repoRoot, rootFiles) {
  const packageFiles = trackedPaths(repoRoot, 'npm/skills');
  const expected = rootFiles.map((entry) => `npm/${entry}`);
  if (JSON.stringify(packageFiles) !== JSON.stringify(expected)) {
    throw new Error('npm/skills must contain exactly the tracked skills/ file set');
  }
  for (const rootRelative of rootFiles) {
    const packageRelative = `npm/${rootRelative}`;
    const rootPath = path.join(repoRoot, rootRelative);
    const packagePath = path.join(repoRoot, packageRelative);
    if (!fs.readFileSync(rootPath).equals(fs.readFileSync(packagePath))) {
      throw new Error(`${packageRelative} differs from ${rootRelative}`);
    }
    const rootExecutable = (fs.statSync(rootPath).mode & 0o111) !== 0;
    const packageExecutable = (fs.statSync(packagePath).mode & 0o111) !== 0;
    if (rootExecutable !== packageExecutable) {
      throw new Error(`${packageRelative} mode differs from ${rootRelative}`);
    }
  }
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
  const catalog = parseCatalog(repoRoot);
  const skillSets = Object.entries(catalog.sets)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, metadata]) => ({
      name,
      ...metadata,
      skills: Object.entries(catalog.skills)
        .filter(([, set]) => set === name)
        .map(([skill]) => skill),
    }));
  return {
    schema_version: 2,
    bundle_version: packageJson.version,
    workspace_schema_version: '2',
    supported_from: ['1.0.2'],
    skill_sets: skillSets,
    files: trackedSkillPaths(repoRoot).map((relative) => fileRecord(packageRoot, relative)),
  };
}

function serialized(manifest) {
  const json = JSON.stringify(manifest, null, 2).replace(
    /^(\s*)"([^"]+)": \[\n((?:\s+"[^"]+"(?:,)?\n)+)\s+\](,?)$/gm,
    (block, indent, key, body, comma) => {
      const items = body
        .trim()
        .split('\n')
        .map((line) => line.trim().replace(/,$/, ''));
      const compact = `${indent}"${key}": [${items.join(', ')}]${comma}`;
      return compact.length <= 100 ? compact : block;
    },
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

module.exports = {
  buildManifest,
  fileRecord,
  parseCatalog,
  serialized,
  validateCatalog,
  validateMirror,
};
