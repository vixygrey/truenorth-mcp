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
  createBuildEvidence,
  createInitialRecord,
  createPublicationEvidence,
  sha256File,
  createRegistryRecord,
  nativeArchiveName,
  validateNotes,
} = require('./release-record.js');

const VERSION = '1.2.3';
const NOTES =
  '# v1.2.3\n\n## Highlights\n\n- Added audit records.\n\n## Migration\n\nNo migration required.\n';

function withFixture(callback) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-release-record-'));
  const assetsDir = path.join(directory, 'assets');
  const evidenceDir = path.join(directory, 'evidence');
  const cargoLock = path.join(directory, 'Cargo.lock');
  const npmLock = path.join(directory, 'package-lock.json');
  const rustToolchain = path.join(directory, 'rust-toolchain.toml');
  const nodeVersionFile = path.join(directory, '.node-version');
  const npmVersionFile = path.join(directory, '.npm-version');
  fs.mkdirSync(assetsDir);
  fs.mkdirSync(evidenceDir);
  fs.writeFileSync(cargoLock, 'cargo lock\n');
  fs.writeFileSync(npmLock, 'npm lock\n');
  fs.writeFileSync(rustToolchain, '[toolchain]\nchannel = "1.88.0"\n');
  fs.writeFileSync(nodeVersionFile, '22.23.3\n');
  fs.writeFileSync(npmVersionFile, '11.15.0\n');
  try {
    for (const [platform] of PLATFORM_PACKAGES) {
      fs.writeFileSync(
        path.join(assetsDir, nativeArchiveName(VERSION, platform)),
        `binary-${platform}`,
      );
      const build = createBuildEvidence({
        platform,
        target: `${platform}-target`,
        commit: 'abc123',
        runnerLabel: `${platform}-runner`,
        runnerOs: platform.startsWith('darwin') ? 'macOS' : 'Linux',
        runnerArch: platform.endsWith('arm64') ? 'ARM64' : 'X64',
        rustcVerbose: 'rustc 1.88.0 (fixture)\nbinary: rustc',
        cargoVersion: 'cargo 1.88.0 (fixture)',
        cargoLockSha256: sha256File(cargoLock),
      });
      fs.writeFileSync(path.join(evidenceDir, `build-${platform}.json`), JSON.stringify(build));
    }
    for (const name of [ROOT_PACKAGE, ...PLATFORM_PACKAGES.map(([, name]) => name)]) {
      const publication = createPublicationEvidence({
        package: name,
        commit: 'abc123',
        nodeVersion: 'v22.23.3',
        npmVersion: '11.15.0',
      });
      fs.writeFileSync(
        path.join(evidenceDir, `publication-${name.replace('/', '__')}.json`),
        JSON.stringify(publication),
      );
    }
    callback({
      assetsDir,
      evidenceDir,
      cargoLock,
      npmLock,
      rustToolchain,
      nodeVersionFile,
      npmVersionFile,
    });
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function initialRecord(fixture) {
  return createInitialRecord({
    version: VERSION,
    tag: `v${VERSION}`,
    commit: 'abc123',
    runUrl: 'https://github.com/example/repo/actions/runs/1',
    repository: 'example/repo',
    notes: NOTES,
    ...fixture,
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
  withFixture((fixture) => {
    const record = initialRecord(fixture);
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
    assert.strictEqual(verification.schema_version, 2);
    assert.strictEqual(verification.npm.status, 'staged_pending_approval');
    assert.strictEqual(verification.npm.packages.length, 5);
    assert.strictEqual(verification.builders.builds.length, 4);
    assert.strictEqual(verification.builders.publications.length, 5);
    assert.deepStrictEqual(verification.dependency_locks, [
      { path: fixture.cargoLock, sha256: sha256File(fixture.cargoLock) },
      { path: fixture.npmLock, sha256: sha256File(fixture.npmLock) },
    ]);
    assert.ok(record.preamble.includes('Registry status: staged_pending_approval.'));
    assert.ok(!record.preamble.includes('NPM_TOKEN'));
  });
});

test('rejects a missing native archive', () => {
  withFixture((fixture) => {
    fs.rmSync(path.join(fixture.assetsDir, nativeArchiveName(VERSION, 'linux-x64')));
    assert.throws(() => initialRecord(fixture), /linux-x64/);
  });
});

test('rejects incomplete or conflicting builder evidence', () => {
  withFixture((fixture) => {
    fs.rmSync(path.join(fixture.evidenceDir, 'build-linux-x64.json'));
    assert.throws(() => initialRecord(fixture), /missing native-build evidence for linux-x64/);
  });

  withFixture((fixture) => {
    const file = path.join(fixture.evidenceDir, 'build-linux-x64.json');
    const evidence = JSON.parse(fs.readFileSync(file, 'utf8'));
    evidence.commit = 'different';
    fs.writeFileSync(file, JSON.stringify(evidence));
    assert.throws(() => initialRecord(fixture), /evidence commit/);
  });

  withFixture((fixture) => {
    const file = path.join(fixture.evidenceDir, 'build-linux-x64.json');
    const evidence = JSON.parse(fs.readFileSync(file, 'utf8'));
    evidence.cargo_lock_sha256 = '0'.repeat(64);
    fs.writeFileSync(file, JSON.stringify(evidence));
    assert.throws(() => initialRecord(fixture), /Cargo.lock digest mismatch/);
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
