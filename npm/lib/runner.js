// The TrueNorth-MCP launcher logic, factored into pure functions so the resolution and
// scaffold behavior is testable without spawning a process.
//
// Requirements: 8.3, 8.4, 8.5, 8.9, 8.10. Design: Part II §7.

'use strict';

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

// The platforms with a published native binary. Windows is out of scope (Requirement 8.8).
const SUPPORTED = new Set(['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64']);

// The state files the init scaffold seeds into a fresh specs/ cockpit.
const SPECS_SEED = {
  'state.yaml': 'active_epic: null\nactive_story: null\nhandoff:\n  next_skill: null\n',
  'release-plan.yaml': 'release:\n  version: 0.1.0\nbuild_order: []\n',
};

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

// Scaffold a fresh specs/ cockpit under `cwd` (Requirement 8.5).
//
// Returns `{ scaffolded: true }` when it created the cockpit, or
// `{ scaffolded: false, reason }` when specs/ already exists (Requirement 8.10). The
// caller reports the skip to stderr.
function scaffoldSpecs(cwd) {
  const specsDir = path.join(cwd, 'specs');
  if (fs.existsSync(specsDir)) {
    return { scaffolded: false, reason: 'specs/ already exists' };
  }
  fs.mkdirSync(specsDir, { recursive: true });
  for (const [name, contents] of Object.entries(SPECS_SEED)) {
    fs.writeFileSync(path.join(specsDir, name), contents);
  }
  return { scaffolded: true };
}

// Run the launcher with an injected environment, for testing.
//
// `deps` carries `argv`, `platform`, `arch`, `cwd`, `resolver`, `spawn`, `stderr`, and
// `exit`, so the tests drive the flow without touching the real process. Returns the exit
// code (the real entry passes it to `process.exit`).
function run(deps) {
  const { argv, platform, arch, cwd, resolver, spawn, stderr, exit } = deps;

  // The `init` subcommand scaffolds the cockpit and returns without spawning.
  if (argv[0] === 'init') {
    const result = scaffoldSpecs(cwd);
    if (!result.scaffolded) {
      stderr(`truenorth-mcp: scaffolding skipped, ${result.reason}\n`);
    }
    return exit(0);
  }

  let binary;
  try {
    binary = resolveBinary(platform, arch, resolver);
  } catch {
    // Requirement 8.4: unsupported platform or missing binary.
    stderr(`truenorth-mcp: no prebuilt binary for ${platform}-${arch}\n`);
    return exit(1);
  }

  const result = spawn(binary, argv, { stdio: 'inherit' });
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
    cwd: process.cwd(),
    resolver: (request) => require.resolve(request),
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
  scaffoldSpecs,
  run,
  realDeps,
};
