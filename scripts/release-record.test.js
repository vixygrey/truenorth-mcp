'use strict';

const assert = require('node:assert');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');

const {
  PLATFORM_PACKAGES,
  ROOT_PACKAGE,
  createInitialRecord,
  createRegistryRecord,
  nativeArchiveName,
  validateNotes,
} = require('./release-record.js');

const VERSION = '1.2.3';
const NOTES =
  '# v1.2.3\n\n## Highlights\n\n- Added audit records.\n\n## Migration\n\nNo migration required.\n';

function withAssets(callback) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-release-record-'));
  try {
    for (const [platform] of PLATFORM_PACKAGES) {
      fs.writeFileSync(
        path.join(directory, nativeArchiveName(VERSION, platform)),
        `binary-${platform}`,
      );
    }
    callback(directory);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function initialRecord(assetsDir) {
  return createInitialRecord({
    version: VERSION,
    tag: `v${VERSION}`,
    commit: 'abc123',
    runUrl: 'https://github.com/example/repo/actions/runs/1',
    repository: 'example/repo',
    notes: NOTES,
    assetsDir,
  });
}

function registryMetadata() {
  return Object.fromEntries(
    [ROOT_PACKAGE, ...PLATFORM_PACKAGES.map(([, name]) => name)].map((name) => [
      name,
      {
        name,
        version: VERSION,
        dist: {
          integrity: `sha512-${name}`,
          tarball: `https://registry.npmjs.org/${name}/-/${name.replace('@truenorth-mcp/', '')}-${VERSION}.tgz`,
        },
      },
    ]),
  );
}

test('validates release notes with highlights and migration sections', () => {
  assert.doesNotThrow(() => validateNotes(VERSION, NOTES));
});

test('rejects malformed or incomplete release notes', () => {
  assert.throws(() => validateNotes(VERSION, '## Highlights\n\n- Missing title.\n'), /start/);
  assert.throws(
    () => validateNotes(VERSION, '# v1.2.3\n\n## Highlights\n\n- Missing migration.\n'),
    /Migration/,
  );
  assert.throws(
    () => validateNotes(VERSION, '# v1.2.3\n\n## Highlights\n\n\n## Migration\n\n\n'),
    /Highlights/,
  );
});

test('creates sorted native checksums and a non-secret staged record', () => {
  withAssets((assetsDir) => {
    const record = initialRecord(assetsDir);
    const lines = record.checksums.trim().split('\n');

    assert.deepStrictEqual(
      lines.map((line) => line.split('  ')[1]),
      PLATFORM_PACKAGES.map(([platform]) => nativeArchiveName(VERSION, platform)),
    );
    for (const [platform] of PLATFORM_PACKAGES) {
      const contents = Buffer.from(`binary-${platform}`);
      const digest = crypto.createHash('sha256').update(contents).digest('hex');
      assert.ok(record.checksums.includes(`${digest}  ${nativeArchiveName(VERSION, platform)}`));
    }
    const verification = JSON.parse(record.verification);
    assert.strictEqual(verification.npm.status, 'staged_pending_approval');
    assert.strictEqual(verification.npm.packages.length, 5);
    assert.ok(record.preamble.includes('Registry status: staged_pending_approval.'));
    assert.ok(!record.preamble.includes('NPM_TOKEN'));
  });
});

test('rejects a missing native archive', () => {
  withAssets((assetsDir) => {
    fs.rmSync(path.join(assetsDir, nativeArchiveName(VERSION, 'linux-x64')));
    assert.throws(() => initialRecord(assetsDir), /linux-x64/);
  });
});

test('creates a registry record only from exact package metadata', () => {
  const record = JSON.parse(
    createRegistryRecord({
      version: VERSION,
      tag: `v${VERSION}`,
      repository: 'example/repo',
      runUrl: 'https://github.com/example/repo/actions/runs/2',
      metadata: registryMetadata(),
    }),
  );

  assert.strictEqual(record.packages.length, 5);
  assert.strictEqual(record.packages[0].name, ROOT_PACKAGE);
  assert.strictEqual(record.provenance_verification, 'npm audit signatures');
});

test('rejects missing, mismatched, or integrity-free registry metadata', () => {
  const missing = registryMetadata();
  delete missing[ROOT_PACKAGE];
  assert.throws(
    () =>
      createRegistryRecord({
        version: VERSION,
        tag: `v${VERSION}`,
        repository: 'example/repo',
        runUrl: 'x',
        metadata: missing,
      }),
    /missing/,
  );

  const mismatched = registryMetadata();
  mismatched[ROOT_PACKAGE].version = '1.2.4';
  assert.throws(
    () =>
      createRegistryRecord({
        version: VERSION,
        tag: `v${VERSION}`,
        repository: 'example/repo',
        runUrl: 'x',
        metadata: mismatched,
      }),
    /does not match/,
  );

  const noIntegrity = registryMetadata();
  delete noIntegrity[ROOT_PACKAGE].dist.integrity;
  assert.throws(
    () =>
      createRegistryRecord({
        version: VERSION,
        tag: `v${VERSION}`,
        repository: 'example/repo',
        runUrl: 'x',
        metadata: noIntegrity,
      }),
    /integrity/,
  );
});
