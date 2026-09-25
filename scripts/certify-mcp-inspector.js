'use strict';

const { execFileSync, spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { createPackedWrapperFixture } = require('./lib/packed-artifact.js');

const REQUIRED_OPERATIONS = [
  'install',
  'initialize',
  'tools_list',
  'resources_list',
  'resource_read',
  'task_mutation',
  'clean_shutdown',
  'workspace_isolation',
];

function main() {
  const options = parseArgs(process.argv.slice(2));
  const repoRoot = path.resolve(__dirname, '..');
  const inspector = path.join(repoRoot, 'node_modules', '.bin', 'mcp-inspector');
  if (!fs.existsSync(inspector)) {
    throw new Error('MCP Inspector is unavailable; run npm install first');
  }

  const beforeStatus = gitStatus(repoRoot);
  const beforePlan = fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml'));
  const fixture = createPackedWrapperFixture(
    options.platformPackage,
    'tn-mcp-inspector-certification-',
  );

  try {
    const catalog = path.join(fixture.work, 'inspector-catalog.json');
    const tools = invokeInspector(inspector, fixture, catalog, 'tools/list');
    const resources = invokeInspector(inspector, fixture, catalog, 'resources/list');
    const state = invokeInspector(inspector, fixture, catalog, 'resources/read', [
      '--uri',
      'truenorth://state',
    ]);
    const mutation = invokeInspector(inspector, fixture, catalog, 'tools/call', [
      '--tool-name',
      'truenorth_record_task',
      '--tool-args-json',
      JSON.stringify({ task_name: 'Certify MCP Inspector', verify_command: 'true' }),
    ]);

    const invocations = [tools, resources, state, mutation];
    const protocolVersion = protocolVersionFrom(invocations);
    const plan = fs.readFileSync(
      path.join(fixture.project, '.agent', 'tasks', 'release-plan.yml'),
      'utf8',
    );
    const afterStatus = gitStatus(repoRoot);
    const afterPlan = fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml'));

    const operations = {
      install: pass('packed wrapper installed with scripts disabled'),
      initialize: outcome(
        Boolean(protocolVersion),
        protocolVersion
          ? `Inspector negotiated MCP ${protocolVersion}`
          : 'Inspector initialization protocol was not observed',
      ),
      tools_list: outcome(
        includesNamed(tools.result?.tools, 'truenorth_record_task'),
        'tools/list includes truenorth_record_task',
      ),
      resources_list: outcome(
        includesUri(resources.result?.resources, 'truenorth://state'),
        'resources/list includes truenorth://state',
      ),
      resource_read: outcome(
        Array.isArray(state.result?.contents) && state.result.contents.length > 0,
        'resources/read returned truenorth://state content',
      ),
      task_mutation: outcome(
        mutation.result?.isError !== true && plan.includes('Certify MCP Inspector'),
        'truenorth_record_task changed only the disposable release plan',
      ),
      clean_shutdown: outcome(
        invocations.every((invocation) => invocation.closed),
        'every Inspector invocation exited zero without a signal after its request',
      ),
      workspace_isolation: outcome(
        beforeStatus === afterStatus && beforePlan === afterPlan,
        'repository status and release plan were unchanged during certification',
      ),
    };

    const evidence = {
      schema_version: 1,
      generated_on: new Date().toISOString().slice(0, 10),
      client: {
        name: 'MCP Inspector CLI',
        version: inspectorVersion(repoRoot),
      },
      server: {
        name: 'truenorth-mcp',
        version: options.expectedVersion,
        transport: 'stdio',
      },
      environment: {
        platform: process.platform,
        architecture: process.arch,
        node: process.version,
        protocol_version: protocolVersion,
      },
      fixture: {
        profile: 'generic',
        packaged_wrapper: true,
        install_scripts: false,
        disposable_workspace: true,
      },
      operations,
      certified: Object.values(operations).every((operation) => operation.status === 'pass'),
    };

    validateEvidence(evidence);
    fs.mkdirSync(options.output, { recursive: true });
    const outputPath = path.join(options.output, 'mcp-inspector.json');
    fs.writeFileSync(outputPath, `${JSON.stringify(evidence, null, 2)}\n`);
    console.log(`MCP Inspector certification wrote ${outputPath}`);
    console.log(
      `MCP Inspector ${evidence.client.version}: ${evidence.certified ? 'certified' : 'failed'}`,
    );
    if (!evidence.certified) {
      process.exitCode = 1;
    }
  } finally {
    fixture.cleanup();
  }
}

function invokeInspector(inspector, fixture, catalog, method, methodArgs = []) {
  const result = spawnSync(
    inspector,
    [
      '--cli',
      fixture.command,
      ...fixture.args,
      '--method',
      method,
      '--format',
      'json',
      '--cwd',
      fixture.project,
      ...methodArgs,
    ],
    {
      cwd: fixture.project,
      env: {
        ...process.env,
        MCP_CATALOG_PATH: catalog,
        MCP_CLIENT_CONFIG_PATH: path.join(fixture.work, 'inspector-client.json'),
        RUST_LOG: 'info',
        TRUENORTH_ROOT: fixture.project,
        TRUENORTH_VERIFY_CMD: 'true',
      },
      encoding: 'utf8',
      maxBuffer: 16 * 1024 * 1024,
    },
  );

  if (result.error) {
    throw new Error(`MCP Inspector ${method} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `MCP Inspector ${method} exited ${result.status}: ${result.stderr.trim() || result.stdout.trim()}`,
    );
  }

  let parsed;
  try {
    parsed = JSON.parse(result.stdout.trim());
  } catch (error) {
    throw new Error(`MCP Inspector ${method} returned invalid JSON: ${error.message}`);
  }

  return {
    ...parsed,
    protocolVersion: parseProtocolVersion(result.stderr),
    closed: result.status === 0 && result.signal === null,
  };
}

function protocolVersionFrom(invocations) {
  return invocations.map((invocation) => invocation.protocolVersion).find(Boolean) ?? null;
}

function parseProtocolVersion(stderr) {
  const match = stderr.match(/protocol_version: ProtocolVersion\("([^"]+)"\)/);
  return match?.[1] ?? null;
}

function includesNamed(items, name) {
  return Array.isArray(items) && items.some((item) => item?.name === name);
}

function includesUri(items, uri) {
  return Array.isArray(items) && items.some((item) => item?.uri === uri);
}

function pass(detail) {
  return { status: 'pass', detail };
}

function outcome(passed, detail) {
  return { status: passed ? 'pass' : 'fail', detail };
}

function validateEvidence(evidence) {
  if (evidence.schema_version !== 1) {
    throw new Error('certification evidence must use schema_version 1');
  }
  for (const name of REQUIRED_OPERATIONS) {
    const operation = evidence.operations?.[name];
    if (!operation || !['pass', 'fail', 'not_exposed'].includes(operation.status)) {
      throw new Error(`certification evidence has no valid ${name} outcome`);
    }
  }
  const expectedCertified = REQUIRED_OPERATIONS.every(
    (name) => evidence.operations[name].status === 'pass',
  );
  if (evidence.certified !== expectedCertified) {
    throw new Error('certified must match the required operation outcomes');
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(evidence.generated_on)) {
    throw new Error('generated_on must be an ISO calendar date');
  }
  return evidence;
}

function inspectorVersion(repoRoot) {
  const manifest = path.join(
    repoRoot,
    'node_modules',
    '@modelcontextprotocol',
    'inspector',
    'package.json',
  );
  return JSON.parse(fs.readFileSync(manifest, 'utf8')).version;
}

function gitStatus(repoRoot) {
  return execFileSync('git', ['status', '--porcelain=v1', '--untracked-files=all'], {
    cwd: repoRoot,
    encoding: 'utf8',
  });
}

function fileDigest(file) {
  if (!fs.existsSync(file)) {
    return null;
  }
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!value) {
      throw new Error(usage());
    }
    if (flag === '--expected-version') {
      options.expectedVersion = value;
    } else if (flag === '--platform-package') {
      options.platformPackage = path.resolve(value);
    } else if (flag === '--output') {
      options.output = path.resolve(value);
    } else {
      throw new Error(usage());
    }
  }
  if (!options.expectedVersion || !options.platformPackage || !options.output) {
    throw new Error(usage());
  }
  return options;
}

function usage() {
  return 'usage: node scripts/certify-mcp-inspector.js --expected-version <version> --platform-package <directory> --output <directory>';
}

module.exports = { REQUIRED_OPERATIONS, parseProtocolVersion, validateEvidence };

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(`MCP Inspector certification failed: ${error.message}`);
    process.exitCode = 1;
  }
}
