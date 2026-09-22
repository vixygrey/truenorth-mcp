'use strict';

const crypto = require('node:crypto');
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
  const verification = {
    schema_version: 1,
    tag: options.tag,
    commit: options.commit,
    repository: options.repository,
    release_workflow_url: options.runUrl,
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
  return `## Audit trail\n\n- Tag: [${options.tag}](https://github.com/${options.repository}/tree/${options.tag})\n- Commit: [${options.commit}](https://github.com/${options.repository}/commit/${options.commit})\n- Verification run: ${options.runUrl}\n\n## Verify native downloads\n\nDownload the archive for your platform and \`SHA256SUMS\`, then run:\n\n\`shasum -a 256 -c SHA256SUMS\`\n\n## npm publication\n\nRegistry status: staged_pending_approval. The release record is updated after registry publication.\n\n${packages.map((pkg) => `- [${pkg.name}@${pkg.version}](${pkg.url})`).join('\n')}\n\nVerify published npm provenance with \`npm audit signatures\`.\n\n${options.notes.trim()}\n`;
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
    'usage: release-record <validate-notes|create-initial-record|create-registry-record>',
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
  createInitialRecord,
  createRegistryRecord,
  nativeArchiveName,
  validateNotes,
};
