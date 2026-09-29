'use strict';

const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const PLATFORM_PACKAGES = [
  ['darwin-arm64', '@truenorth-mcp/darwin-arm64'],
  ['darwin-x64', '@truenorth-mcp/darwin-x64'],
  ['linux-arm64', '@truenorth-mcp/linux-arm64'],
  ['linux-x64', '@truenorth-mcp/linux-x64'],
];
const ROOT_PACKAGE = 'truenorth-mcp';

function nativeArchiveName(version, platform) {
  return `truenorth-mcp-v${version}-${platform}.tar.gz`;
}

function packageUrl(name, version) {
  return `https://www.npmjs.com/package/${name}/v/${version}`;
}

function validateNotes(version, text) {
  const title = `# v${version}`;
  const highlights = section(text, 'Highlights');
  const migration = section(text, 'Migration');

  if (!text.startsWith(`${title}\n`)) {
    throw new Error(`release notes must start with \`${title}\``);
  }
  if (!highlights || !highlights.trim()) {
    throw new Error('release notes must include a non-empty `## Highlights` section');
  }
  if (!migration || !migration.trim()) {
    throw new Error('release notes must include a non-empty `## Migration` section');
  }
}

function section(text, heading) {
  const marker = `## ${heading}`;
  const headingStart = text.indexOf(marker);
  if (headingStart === -1) {
    return undefined;
  }
  const contentStart = text.indexOf('\n', headingStart) + 1;
  const nextHeading = text.indexOf('\n## ', contentStart);
  return text.slice(contentStart, nextHeading === -1 ? text.length : nextHeading);
}
function sha256File(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function pinnedVersion(file) {
  const value = fs.readFileSync(file, 'utf8').trim();
  if (!value) {
    throw new Error(`toolchain pin is empty: ${file}`);
  }
  return value;
}

function pinnedRustVersion(file) {
  const text = fs.readFileSync(file, 'utf8');
  const match = text.match(/^\s*channel\s*=\s*"([^"]+)"\s*$/m);
  if (!match) {
    throw new Error(`Rust toolchain pin has no channel: ${file}`);
  }
  return match[1];
}

function commandOutput(command, args) {
  return execFileSync(command, args, { encoding: 'utf8' }).trim();
}

function createBuildEvidence(options) {
  return {
    schema_version: 1,
    kind: 'native-build',
    platform: options.platform,
    target: options.target,
    commit: options.commit,
    runner: {
      label: options.runnerLabel,
      os: options.runnerOs,
      arch: options.runnerArch,
      image_os: options.imageOs || null,
      image_version: options.imageVersion || null,
    },
    rust: {
      rustc_verbose: options.rustcVerbose,
      cargo: options.cargoVersion,
    },
    cargo_lock_sha256: options.cargoLockSha256,
  };
}

function createPublicationEvidence(options) {
  return {
    schema_version: 1,
    kind: 'npm-publication',
    package: options.package,
    commit: options.commit,
    node: options.nodeVersion,
    npm: options.npmVersion,
  };
}

function loadEvidence(directory) {
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.json'))
    .map((entry) => JSON.parse(fs.readFileSync(path.join(directory, entry.name), 'utf8')));
}

function uniqueRecords(records, key, expected, kind) {
  const byKey = new Map();
  for (const record of records.filter((entry) => entry.kind === kind)) {
    const value = record[key];
    if (!expected.includes(value)) {
      throw new Error(`unexpected ${kind} ${key}: ${value}`);
    }
    if (byKey.has(value)) {
      throw new Error(`duplicate ${kind} ${key}: ${value}`);
    }
    byKey.set(value, record);
  }
  for (const value of expected) {
    if (!byKey.has(value)) {
      throw new Error(`missing ${kind} evidence for ${value}`);
    }
  }
  return expected.map((value) => byKey.get(value));
}

function validatedEvidence(options, cargoLockSha256) {
  const evidence = loadEvidence(options.evidenceDir);
  const platforms = PLATFORM_PACKAGES.map(([platform]) => platform);
  const packageNames = [ROOT_PACKAGE, ...PLATFORM_PACKAGES.map(([, name]) => name)];
  const builds = uniqueRecords(evidence, 'platform', platforms, 'native-build');
  const publications = uniqueRecords(evidence, 'package', packageNames, 'npm-publication');
  const rustVersion = pinnedRustVersion(options.rustToolchain);
  const nodeVersion = `v${pinnedVersion(options.nodeVersionFile)}`;
  const npmVersion = pinnedVersion(options.npmVersionFile);

  for (const record of [...builds, ...publications]) {
    if (record.commit !== options.commit) {
      throw new Error(`evidence commit does not match release commit: ${record.commit}`);
    }
  }
  for (const record of builds) {
    if (record.cargo_lock_sha256 !== cargoLockSha256) {
      throw new Error(`Cargo.lock digest mismatch for ${record.platform}`);
    }
    if (!record.rust?.rustc_verbose?.startsWith(`rustc ${rustVersion} `)) {
      throw new Error(`Rust version mismatch for ${record.platform}`);
    }
  }
  for (const record of publications) {
    if (record.node !== nodeVersion) {
      throw new Error(`Node version mismatch for ${record.package}`);
    }
    if (record.npm !== npmVersion) {
      throw new Error(`npm version mismatch for ${record.package}`);
    }
  }
  return { builds, publications };
}

function createInitialRecord(options) {
  validateNotes(options.version, options.notes);
  const assets = PLATFORM_PACKAGES.map(([platform]) => {
    const name = nativeArchiveName(options.version, platform);
    const file = path.join(options.assetsDir, name);
    const bytes = fs.readFileSync(file);
    return {
      platform,
      name,
      bytes: bytes.length,
      sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    };
  });
  const checksumLines = assets.map((asset) => `${asset.sha256}  ${asset.name}`);
  const packageRecords = [ROOT_PACKAGE, ...PLATFORM_PACKAGES.map(([, name]) => name)].map(
    (name) => ({
      name,
      version: options.version,
      url: packageUrl(name, options.version),
    }),
  );
  const dependencyLocks = [
    { path: options.cargoLock, sha256: sha256File(options.cargoLock) },
    { path: options.npmLock, sha256: sha256File(options.npmLock) },
  ];
  const builders = validatedEvidence(options, dependencyLocks[0].sha256);
  const verification = {
    schema_version: 2,
    tag: options.tag,
    commit: options.commit,
    repository: options.repository,
    release_workflow_url: options.runUrl,
    dependency_locks: dependencyLocks,
    builders,
    native_assets: assets,
    npm: {
      status: 'staged_pending_approval',
      packages: packageRecords,
      provenance_verification: 'npm audit signatures',
    },
  };

  return {
    checksums: `${checksumLines.join('\n')}\n`,
    verification: `${JSON.stringify(verification, null, 2)}\n`,
    preamble: releasePreamble(options, packageRecords),
  };
}

function releasePreamble(options, packages) {
  return `## Audit trail\n\n- Tag: [${options.tag}](https://github.com/${options.repository}/tree/${options.tag})\n- Commit: [${options.commit}](https://github.com/${options.repository}/commit/${options.commit})\n- Verification run: ${options.runUrl}\n\n## Verify native downloads\n\nDownload the archive for your platform and \`SHA256SUMS\`, then run:\n\n\`shasum -a 256 -c SHA256SUMS\`\n\nTo cryptographically verify the build provenance attestation:\n\n\`gh attestation verify <archive-filename> --owner ${options.repository.split('/')[0]}\`\n\n## npm publication\n\nRegistry status: staged_pending_approval. The release record is updated after registry publication.\n\n${packages.map((pkg) => `- [${pkg.name}@${pkg.version}](${pkg.url})`).join('\n')}\n\nVerify published npm provenance with \`npm audit signatures\`.\n\n${options.notes.trim()}\n`;
}

function createRegistryRecord(options) {
  const expected = [ROOT_PACKAGE, ...PLATFORM_PACKAGES.map(([, name]) => name)];
  const packages = expected.map((name) => {
    const metadata = options.metadata[name];
    if (!metadata) {
      throw new Error(`registry metadata is missing for ${name}`);
    }
    if (metadata.name !== name || metadata.version !== options.version) {
      throw new Error(`registry metadata for ${name} does not match version ${options.version}`);
    }
    if (typeof metadata.dist?.integrity !== 'string' || !metadata.dist.integrity) {
      throw new Error(`registry metadata for ${name} has no dist.integrity`);
    }
    if (typeof metadata.dist?.tarball !== 'string' || !metadata.dist.tarball) {
      throw new Error(`registry metadata for ${name} has no dist.tarball`);
    }
    return {
      name,
      version: metadata.version,
      tarball: metadata.dist.tarball,
      integrity: metadata.dist.integrity,
      url: packageUrl(name, options.version),
    };
  });

  return `${JSON.stringify(
    {
      schema_version: 1,
      tag: options.tag,
      repository: options.repository,
      registry_verification_workflow_url: options.runUrl,
      packages,
      provenance_verification: 'npm audit signatures',
    },
    null,
    2,
  )}\n`;
}

function main(argv) {
  const [command, ...rest] = argv;
  const args = parseArgs(rest);
  if (command === 'validate-notes') {
    validateNotes(required(args, 'version'), fs.readFileSync(required(args, 'notes'), 'utf8'));
    return;
  }
  if (command === 'capture-build-evidence') {
    const evidence = createBuildEvidence({
      platform: required(args, 'platform'),
      target: required(args, 'target'),
      commit: required(args, 'commit'),
      runnerLabel: required(args, 'runner-label'),
      runnerOs: required(args, 'runner-os'),
      runnerArch: required(args, 'runner-arch'),
      imageOs: process.env.ImageOS,
      imageVersion: process.env.ImageVersion,
      rustcVerbose: commandOutput('rustc', ['-Vv']),
      cargoVersion: commandOutput('cargo', ['-V']),
      cargoLockSha256: sha256File(required(args, 'cargo-lock')),
    });
    fs.writeFileSync(required(args, 'output'), `${JSON.stringify(evidence, null, 2)}\n`);
    return;
  }
  if (command === 'capture-publication-evidence') {
    const evidence = createPublicationEvidence({
      package: required(args, 'package'),
      commit: required(args, 'commit'),
      nodeVersion: commandOutput('node', ['--version']),
      npmVersion: commandOutput('npm', ['--version']),
    });
    fs.writeFileSync(required(args, 'output'), `${JSON.stringify(evidence, null, 2)}\n`);
    return;
  }
  if (command === 'create-initial-record') {
    const output = required(args, 'output-dir');
    fs.mkdirSync(output, { recursive: true });
    const record = createInitialRecord({
      version: required(args, 'version'),
      tag: required(args, 'tag'),
      commit: required(args, 'commit'),
      runUrl: required(args, 'run-url'),
      repository: required(args, 'repository'),
      notes: fs.readFileSync(required(args, 'notes'), 'utf8'),
      assetsDir: required(args, 'assets-dir'),
      evidenceDir: required(args, 'evidence-dir'),
      cargoLock: required(args, 'cargo-lock'),
      npmLock: required(args, 'npm-lock'),
      rustToolchain: required(args, 'rust-toolchain'),
      nodeVersionFile: required(args, 'node-version-file'),
      npmVersionFile: required(args, 'npm-version-file'),
    });
    fs.writeFileSync(path.join(output, 'SHA256SUMS'), record.checksums);
    fs.writeFileSync(path.join(output, 'RELEASE-VERIFICATION.json'), record.verification);
    fs.writeFileSync(path.join(output, 'release-preamble.md'), record.preamble);
    return;
  }
  if (command === 'create-registry-record') {
    const metadataDir = required(args, 'metadata-dir');
    const metadata = Object.fromEntries(
      [ROOT_PACKAGE, ...PLATFORM_PACKAGES.map(([, name]) => name)].map((name) => [
        name,
        JSON.parse(
          fs.readFileSync(path.join(metadataDir, `${name.replace('/', '__')}.json`), 'utf8'),
        ),
      ]),
    );
    const output = required(args, 'output');
    fs.writeFileSync(
      output,
      createRegistryRecord({
        version: required(args, 'version'),
        tag: required(args, 'tag'),
        repository: required(args, 'repository'),
        runUrl: required(args, 'run-url'),
        metadata,
      }),
    );
    return;
  }
  throw new Error(
    'usage: release-record <validate-notes|capture-build-evidence|capture-publication-evidence|create-initial-record|create-registry-record>',
  );
}

function parseArgs(argv) {
  if (argv.length % 2 !== 0) {
    throw new Error('arguments must be supplied as --name value pairs');
  }
  const args = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    if (!key.startsWith('--') || !argv[index + 1]) {
      throw new Error('arguments must be supplied as --name value pairs');
    }
    args[key.slice(2)] = argv[index + 1];
  }
  return args;
}

function required(args, key) {
  if (!args[key]) {
    throw new Error(`missing --${key}`);
  }
  return args[key];
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(`release-record: ${error.message}`);
    process.exitCode = 1;
  }
}

module.exports = {
  PLATFORM_PACKAGES,
  ROOT_PACKAGE,
  createBuildEvidence,
  createInitialRecord,
  createPublicationEvidence,
  sha256File,
  createRegistryRecord,
  nativeArchiveName,
  validateNotes,
};
