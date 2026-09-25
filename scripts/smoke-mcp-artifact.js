'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { startSession } = require('./lib/mcp-session.js');
const { createPackedWrapperFixture } = require('./lib/packed-artifact.js');

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.platformPackage) {
    await smokePackedWrapper(options.expectedVersion, options.platformPackage);
  } else {
    await smokeCommand(options.expectedVersion, options.command, options.args);
  }
}

async function smokePackedWrapper(expectedVersion, platformPackage) {
  const fixture = createPackedWrapperFixture(platformPackage, 'tn-packed-mcp-smoke-');
  try {
    await smokeCommand(expectedVersion, fixture.command, fixture.args, fixture.project, true);
  } finally {
    fixture.cleanup();
  }
}

async function smokeCommand(
  expectedVersion,
  command,
  args,
  existingRoot,
  checkBundledSkill = false,
) {
  const root = existingRoot ?? fs.mkdtempSync(path.join(os.tmpdir(), 'tn-mcp-smoke-'));
  const ownsRoot = !existingRoot;
  if (ownsRoot) {
    for (const marker of ['.agent', 'specs', 'skills']) {
      fs.mkdirSync(path.join(root, marker));
    }
  }
  const session = startSession(command, args, root);

  try {
    const initialize = await session.request('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'truenorth-artifact-smoke', version: '1.0.0' },
    });
    assertEqual(initialize.serverInfo?.name, 'truenorth-mcp', 'serverInfo.name');
    assertEqual(initialize.serverInfo?.version, expectedVersion, 'serverInfo.version');
    assertObject(initialize.capabilities?.tools, 'initialize capabilities.tools');
    assertObject(initialize.capabilities?.resources, 'initialize capabilities.resources');

    session.notify('notifications/initialized', {});

    const tools = await session.request('tools/list', {});
    assertContains(tools.tools, 'name', 'truenorth_verify_gate', 'tools/list');

    const resources = await session.request('resources/list', {});
    assertContains(resources.resources, 'uri', 'truenorth://state', 'resources/list');

    if (checkBundledSkill) {
      const skill = await session.request('tools/call', {
        name: 'get_skill',
        arguments: { name: 'using-truenorth', tier: 'lean' },
      });
      assertObject(skill, 'get_skill result');
    }

    await session.close();
    console.log(`MCP artifact smoke passed for truenorth-mcp ${expectedVersion}.`);
  } finally {
    await session.terminate();
    if (ownsRoot) {
      fs.rmSync(root, { recursive: true, force: true });
    }
  }
}

function parseArgs(argv) {
  if (argv[0] !== '--expected-version' || !argv[1]) {
    throw new Error(usage());
  }

  if (argv[2] === '--packed-wrapper' && argv[3] && argv.length === 4) {
    return { expectedVersion: argv[1], platformPackage: path.resolve(argv[3]) };
  }

  if (argv[2] === '--' && argv[3]) {
    return { expectedVersion: argv[1], command: argv[3], args: argv.slice(4) };
  }

  throw new Error(usage());
}

function usage() {
  return [
    'usage:',
    'node scripts/smoke-mcp-artifact.js --expected-version <version> -- <command> [arguments...]',
    'node scripts/smoke-mcp-artifact.js --expected-version <version> --packed-wrapper <platform-package-directory>',
  ].join('\n');
}

function assertEqual(actual, expected, field) {
  if (actual !== expected) {
    throw new Error(`${field} is ${actual ?? 'missing'}, expected ${expected}`);
  }
}

function assertObject(value, field) {
  if (!value || typeof value !== 'object') {
    throw new Error(`${field} is missing`);
  }
}

function assertContains(items, field, expected, method) {
  if (!Array.isArray(items) || !items.some((item) => item?.[field] === expected)) {
    throw new Error(`${method} does not include ${expected}`);
  }
}

main().catch((error) => {
  console.error(`MCP artifact smoke failed: ${error.message}`);
  process.exitCode = 1;
});
