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
    const tasks = path.join(root, '.agent', 'tasks');
    fs.mkdirSync(tasks);
    fs.writeFileSync(
      path.join(tasks, 'state.yml'),
      'active_flow: null\nactive_group: null\nactive_task: null\nphase: null\ntdd:\n  step: refactor\nhandoff:\n  next_skill: null\n  context: null\n  group: null\ngit:\n  branch: main\n',
    );
  }
  const session = startSession(command, args, root);

  try {
    const clientContext = {
      'io.modelcontextprotocol/protocolVersion': '2026-07-28',
      'io.modelcontextprotocol/clientInfo': {
        name: 'truenorth-artifact-smoke',
        version: '1.0.0',
      },
      'io.modelcontextprotocol/clientCapabilities': {},
    };
    const params = (value = {}) => ({ ...value, _meta: clientContext });
    const discovery = await session.request('server/discover', params());
    const serverInfo = discovery._meta?.['io.modelcontextprotocol/serverInfo'];

    assertIncludes(discovery.supportedVersions, '2026-07-28', 'server/discover');
    assertEqual(serverInfo?.name, 'truenorth-mcp', 'serverInfo.name');
    assertEqual(serverInfo?.version, expectedVersion, 'serverInfo.version');
    assertObject(discovery.capabilities?.tools, 'server/discover capabilities.tools');
    assertObject(discovery.capabilities?.resources, 'server/discover capabilities.resources');
    assertCacheMetadata(discovery, 'server/discover');

    const tools = await session.request('tools/list', params());
    assertContains(tools.tools, 'name', 'truenorth_verify_gate', 'tools/list');

    const resources = await session.request('resources/list', params());
    assertContains(resources.resources, 'uri', 'truenorth://state', 'resources/list');
    assertCacheMetadata(resources, 'resources/list');

    const templates = await session.request('resources/templates/list', params());
    assertCacheMetadata(templates, 'resources/templates/list');

    const state = await session.request('resources/read', params({ uri: 'truenorth://state' }));
    assertContains(state.contents, 'uri', 'truenorth://state', 'resources/read');
    assertCacheMetadata(state, 'resources/read');

    if (checkBundledSkill) {
      const skill = await session.request(
        'tools/call',
        params({
          name: 'get_skill',
          arguments: { name: 'using-truenorth', tier: 'lean' },
        }),
      );
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

function assertIncludes(items, expected, method) {
  if (!Array.isArray(items) || !items.includes(expected)) {
    throw new Error(`${method} does not include ${expected}`);
  }
}

function assertCacheMetadata(result, method) {
  assertEqual(result.ttlMs, 0, `${method} ttlMs`);
  assertEqual(result.cacheScope, 'private', `${method} cacheScope`);
}

main().catch((error) => {
  console.error(`MCP artifact smoke failed: ${error.message}`);
  process.exitCode = 1;
});
