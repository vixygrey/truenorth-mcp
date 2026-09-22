'use strict';

const fs = require('node:fs');
const path = require('node:path');

const PLATFORM_PACKAGES = ['darwin-arm64', 'darwin-x64', 'linux-arm64', 'linux-x64'];
const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(?:(?:0|[1-9]\d*)|(?:\d*[A-Za-z-][0-9A-Za-z-]*))(?:\.(?:(?:0|[1-9]\d*)|(?:\d*[A-Za-z-][0-9A-Za-z-]*)))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

function checkReleaseVersion(root, expectedVersion) {
  if (!SEMVER.test(expectedVersion)) {
    return [`expected version must be SemVer without a leading v: ${expectedVersion}`];
  }

  const errors = [];
  const displayPath = (file) => path.relative(root, file);
  const cargoManifest = path.join(root, 'runtime', 'Cargo.toml');
  const cargoLock = path.join(root, 'runtime', 'Cargo.lock');
  const wrapperManifest = path.join(root, 'npm', 'package.json');

  expectVersion(
    errors,
    displayPath,
    cargoManifest,
    'package version',
    cargoPackageVersion(readFile(cargoManifest)),
    expectedVersion,
  );
  expectVersion(
    errors,
    displayPath,
    cargoLock,
    'truenorth-mcp lock version',
    cargoLockVersion(readFile(cargoLock)),
    expectedVersion,
  );

  const wrapper = readJson(wrapperManifest);
  expectVersion(
    errors,
    displayPath,
    wrapperManifest,
    'package version',
    wrapper.version,
    expectedVersion,
  );

  for (const platform of PLATFORM_PACKAGES) {
    const packageName = `@truenorth-mcp/${platform}`;
    expectVersion(
      errors,
      displayPath,
      wrapperManifest,
      `optional dependency ${packageName}`,
      wrapper.optionalDependencies?.[packageName],
      expectedVersion,
    );

    const platformManifest = path.join(root, 'npm', 'packages', platform, 'package.json');
    expectVersion(
      errors,
      displayPath,
      platformManifest,
      'package version',
      readJson(platformManifest).version,
      expectedVersion,
    );
  }

  return errors;
}

function cargoPackageVersion(content) {
  const header = '[package]';
  const start = content.indexOf(header);
  if (start === -1) {
    return undefined;
  }

  const afterHeader = content.slice(start + header.length);
  const nextSection = afterHeader.search(/^\[/m);
  return quotedVersion(nextSection === -1 ? afterHeader : afterHeader.slice(0, nextSection));
}

function cargoLockVersion(content) {
  const packageSection = content.match(
    /\[\[package\]\]\s*\nname = "truenorth-mcp"\s*\nversion = "([^"]+)"/,
  );
  return packageSection?.[1];
}

function quotedVersion(content) {
  return content?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
}

function expectVersion(errors, displayPath, file, field, actual, expected) {
  if (actual !== expected) {
    errors.push(`${displayPath(file)}: ${field} is ${actual ?? 'missing'}, expected ${expected}`);
  }
}

function readFile(file) {
  return fs.readFileSync(file, 'utf8');
}

function readJson(file) {
  return JSON.parse(readFile(file));
}

function main() {
  const expectedVersion = process.argv[2];
  const errors = checkReleaseVersion(process.cwd(), expectedVersion);
  if (errors.length === 0) {
    console.log(`Release metadata matches ${expectedVersion}.`);
    return;
  }

  console.error('Release version preflight failed:');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exitCode = 1;
}

if (require.main === module) {
  main();
}

module.exports = { checkReleaseVersion };
