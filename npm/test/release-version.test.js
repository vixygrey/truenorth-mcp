'use strict';

const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');

const { checkReleaseVersion } = require('../../scripts/check-release-version.js');

const VERSION = '1.0.0';
const PLATFORMS = ['darwin-arm64', 'darwin-x64', 'linux-arm64', 'linux-x64'];

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-release-version-'));
  write(root, 'runtime/Cargo.toml', `[package]\nname = "truenorth-mcp"\nversion = "${VERSION}"\n`);
  write(
    root,
    'runtime/Cargo.lock',
    `version = 4\n\n[[package]]\nname = "truenorth-mcp"\nversion = "${VERSION}"\n`,
  );
  writeJson(root, 'npm/package.json', {
    name: 'truenorth-mcp',
    version: VERSION,
    optionalDependencies: Object.fromEntries(
      PLATFORMS.map((platform) => [`@truenorth-mcp/${platform}`, VERSION]),
    ),
  });
  for (const platform of PLATFORMS) {
    writeJson(root, `npm/packages/${platform}/package.json`, {
      name: `@truenorth-mcp/${platform}`,
      version: VERSION,
    });
  }
  return root;
}

function write(root, file, content) {
  const target = path.join(root, file);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content);
}

function writeJson(root, file, value) {
  write(root, file, `${JSON.stringify(value, null, 2)}\n`);
}

function withFixture(callback) {
  const root = fixture();
  try {
    callback(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test('accepts synchronized release metadata', () => {
  withFixture((root) => {
    assert.deepStrictEqual(checkReleaseVersion(root, VERSION), []);
  });
});

test('rejects an invalid release version', () => {
  withFixture((root) => {
    assert.deepStrictEqual(checkReleaseVersion(root, 'v1.0.0'), [
      'expected version must be SemVer without a leading v: v1.0.0',
    ]);
  });
});

test('rejects a mismatched Cargo manifest version', () => {
  withFixture((root) => {
    write(root, 'runtime/Cargo.toml', '[package]\nname = "truenorth-mcp"\nversion = "0.9.0"\n');
    assert.match(
      checkReleaseVersion(root, VERSION).join('\n'),
      /runtime\/Cargo\.toml: package version is 0\.9\.0, expected 1\.0\.0/,
    );
  });
});

test('rejects a mismatched Cargo lock version', () => {
  withFixture((root) => {
    write(
      root,
      'runtime/Cargo.lock',
      'version = 4\n\n[[package]]\nname = "truenorth-mcp"\nversion = "0.9.0"\n',
    );
    assert.match(
      checkReleaseVersion(root, VERSION).join('\n'),
      /runtime\/Cargo\.lock: truenorth-mcp lock version is 0\.9\.0, expected 1\.0\.0/,
    );
  });
});

test('rejects a mismatched root wrapper version', () => {
  withFixture((root) => {
    const manifest = path.join(root, 'npm/package.json');
    const wrapper = JSON.parse(fs.readFileSync(manifest, 'utf8'));
    wrapper.version = '0.9.0';
    fs.writeFileSync(manifest, `${JSON.stringify(wrapper, null, 2)}\n`);
    assert.match(
      checkReleaseVersion(root, VERSION).join('\n'),
      /npm\/package\.json: package version is 0\.9\.0, expected 1\.0\.0/,
    );
  });
});

test('rejects a mismatched optional dependency version', () => {
  withFixture((root) => {
    const manifest = path.join(root, 'npm/package.json');
    const wrapper = JSON.parse(fs.readFileSync(manifest, 'utf8'));
    wrapper.optionalDependencies['@truenorth-mcp/linux-arm64'] = '0.9.0';
    fs.writeFileSync(manifest, `${JSON.stringify(wrapper, null, 2)}\n`);
    assert.match(
      checkReleaseVersion(root, VERSION).join('\n'),
      /optional dependency @truenorth-mcp\/linux-arm64 is 0\.9\.0, expected 1\.0\.0/,
    );
  });
});

test('rejects a mismatched platform package version', () => {
  withFixture((root) => {
    writeJson(root, 'npm/packages/linux-arm64/package.json', {
      name: '@truenorth-mcp/linux-arm64',
      version: '0.9.0',
    });
    assert.match(
      checkReleaseVersion(root, VERSION).join('\n'),
      /npm\/packages\/linux-arm64\/package\.json: package version is 0\.9\.0, expected 1\.0\.0/,
    );
  });
});
