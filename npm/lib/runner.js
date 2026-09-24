// The TrueNorth-MCP launcher logic, factored into pure functions so the binary
// resolution is testable without spawning a process.
//
// Requirements: 8.3, 8.4, 8.9. Design: Part II §7.

'use strict';

const { spawnSync } = require('node:child_process');
const path = require('node:path');

// The platforms with a published native binary. Windows is out of scope (Requirement 8.8).
const SUPPORTED = new Set(['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64']);

// The platform package name for a platform and arch, for example
// `@truenorth-mcp/darwin-arm64`.
function platformPackage(platform, arch) {
  return `@truenorth-mcp/${platform}-${arch}`;
}

// Whether a platform and arch pair has a published binary.
function isSupported(platform, arch) {
  return SUPPORTED.has(`${platform}-${arch}`);
}

// Resolve the native binary path for a platform and arch.
//
// `resolver` is `require.resolve`-compatible; the tests pass a fake. Returns the resolved
// path, or throws when the platform package is not installed or the pair is unsupported.
function resolveBinary(platform, arch, resolver) {
  if (!isSupported(platform, arch)) {
    throw new Error(`no prebuilt binary for ${platform}-${arch}`);
  }
  const pkg = platformPackage(platform, arch);
  // The platform package ships the binary as `truenorth-mcp` at its root.
  return resolver(`${pkg}/truenorth-mcp`);
}

// `deps` carries `argv`, `platform`, `arch`, `packageDir`, `resolver`, `spawn`, `stderr`, and
// `exit`, so the tests drive the flow without touching the real process. Returns the exit code
// (the real entry passes it to `process.exit`).
function run(deps) {
  const { argv, platform, arch, packageDir, resolver, spawn, stderr, exit } = deps;

  if (platform === 'win32') {
    stderr(
      'truenorth-mcp: native Windows is not supported in v1. ' +
        'Run TrueNorth-MCP in WSL: ' +
        'https://github.com/vixygrey/truenorth-mcp/wiki/Install-and-connect#windows\n',
    );
    return exit(1);
  }

  const binaryArgs =
    argv[0] === 'init' ? [...argv, '--skills-dir', path.join(packageDir, 'skills')] : argv;

  let binary;
  try {
    binary = resolveBinary(platform, arch, resolver);
  } catch {
    // Requirement 8.4: unsupported platform or missing binary.
    stderr(`truenorth-mcp: no prebuilt binary for ${platform}-${arch}\n`);
    return exit(1);
  }

  const result = spawn(binary, binaryArgs, { stdio: 'inherit' });
  if (result.error) {
    // Requirement 8.9: the resolved binary failed to spawn.
    stderr(`truenorth-mcp: failed to start the binary: ${result.error.message}\n`);
    return exit(1);
  }
  // Requirement 8.3: propagate the child's exit code.
  return exit(result.status == null ? 1 : result.status);
}

// The real dependencies for the entrypoint.
function realDeps() {
  return {
    argv: process.argv.slice(2),
    platform: process.platform,
    arch: process.arch,
    resolver: (request) => require.resolve(request),
    packageDir: path.resolve(__dirname, '..'),
    spawn: spawnSync,
    stderr: (text) => process.stderr.write(text),
    exit: (code) => process.exit(code),
  };
}

module.exports = {
  SUPPORTED,
  platformPackage,
  isSupported,
  resolveBinary,
  run,
  realDeps,
};
