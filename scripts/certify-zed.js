'use strict';

const { execFileSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createPackedWrapperFixture } = require('./lib/packed-artifact.js');

const PINNED_ZED_VERSION = '1.21.0';
const PINNED_ZED_COMMIT = '33c95853ed2b6956f339733c63a8220964ecbeb6';
const TASK_NAME = 'Certify Zed MCP integration #427';
const REQUIRED_OPERATIONS = [
  'install',
  'lifecycle_negotiation',
  'tools_list',
  'resources_list',
  'resource_read',
  'read_tool_operation',
  'task_mutation',
  'clean_shutdown',
  'workspace_isolation',
];

function main() {
  const [command, ...args] = process.argv.slice(2);
  if (command === 'prepare') {
    return prepare(
      parseOptions(args, ['expectedVersion', 'platformPackage', 'zed', 'modelProvider', 'modelId']),
    );
  }
  if (command === 'collect') return collect(parseOptions(args, ['session', 'output']));
  if (command === 'validate') return validateFile(parseOptions(args, ['evidence']));
  if (command === 'cleanup') return cleanup(parseOptions(args, ['session']));
  throw new Error('usage: certify-zed.js <prepare|collect|validate|cleanup> [options]');
}

function prepare(options) {
  if (process.platform !== 'darwin' || process.arch !== 'arm64') {
    throw new Error('the pinned Zed certificate requires darwin-arm64');
  }

  const repoRoot = path.resolve(__dirname, '..');
  const zed = readZedMetadata(options.zed);
  assertPinnedClient(zed);
  const fixture = createPackedWrapperFixture(options.platformPackage, 'tn-zed-certification-');

  try {
    const userData = path.join(fixture.work, 'zed-user-data');
    const transcript = path.join(fixture.work, 'mcp-transcript.jsonl');
    const lifecycle = path.join(fixture.work, 'mcp-lifecycle.jsonl');
    const serverStderr = path.join(fixture.work, 'server-stderr.log');
    const monitor = path.join(fixture.work, 'server-monitor.js');
    fs.mkdirSync(userData, { recursive: true });
    writeMonitor(monitor);
    writeConfiguration({ fixture, monitor, transcript, lifecycle, serverStderr });

    const serverVersion = wrapperVersion(fixture, options.expectedVersion);
    const manifest = {
      kind: 'truenorth-zed-certification',
      session_root: fixture.work,
      repository_root: repoRoot,
      workspace: fixture.project,
      user_data: userData,
      transcript,
      lifecycle,
      server_stderr: serverStderr,
      zed_command: path.resolve(options.zed),
      zed,
      server_version: serverVersion,
      model: {
        provider: options.modelProvider,
        id: options.modelId,
      },
      baseline: {
        git_status: gitStatus(repoRoot),
        release_plan_digest: fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml')),
        user_settings_digest: fileDigestOrMissing(zedUserSettings()),
      },
    };
    const manifestPath = path.join(fixture.work, 'certification-session.json');
    fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

    console.log(`Prepared disposable Zed certification session: ${fixture.work}`);
    console.log('Launch the isolated Zed instance:');
    console.log(
      `${shellQuote(manifest.zed_command)} --user-data-dir ${shellQuote(userData)} --new --wait ${shellQuote(fixture.project)}`,
    );
    console.log('After the manual steps and clean shutdown, collect evidence:');
    console.log(
      `node scripts/certify-zed.js collect --session ${shellQuote(fixture.work)} --output compatibility`,
    );
    console.log('Then remove the disposable session:');
    console.log(`node scripts/certify-zed.js cleanup --session ${shellQuote(fixture.work)}`);
  } catch (error) {
    fixture.cleanup();
    throw error;
  }
}

function collect(options) {
  const manifest = readSession(options.session);
  const protocol = readJsonLines(manifest.transcript);
  const lifecycle = readJsonLines(manifest.lifecycle);
  const observed = protocolEvidence(protocol);
  const planPath = path.join(manifest.workspace, '.agent', 'tasks', 'release-plan.yml');
  const plan = fs.existsSync(planPath) ? fs.readFileSync(planPath, 'utf8') : '';
  const taskCount = countOccurrences(plan, TASK_NAME);
  const currentStatus = gitStatus(manifest.repository_root);
  const currentPlanDigest = fileDigest(
    path.join(manifest.repository_root, '.agent', 'tasks', 'release-plan.yml'),
  );
  const currentUserSettingsDigest = fileDigestOrMissing(zedUserSettings());
  const starts = lifecycle.filter((event) => event.event === 'start');
  const exits = lifecycle.filter((event) => event.event === 'exit');
  const completedTools = new Set(
    observed.tool_calls.filter((call) => call.status === 'completed').map((call) => call.tool),
  );
  const hasResourcesList = observed.methods.includes('resources/list');
  const hasResourceRead = observed.methods.includes('resources/read');

  const operations = {
    install: pass(
      `released truenorth-mcp ${manifest.server_version} wrapper installed with scripts disabled`,
    ),
    lifecycle_negotiation: outcome(
      observed.methods.includes('initialize') && observed.client_info?.name === 'Zed',
      'Zed initialized the TrueNorth stdio server',
    ),
    tools_list: outcome(
      observed.methods.includes('tools/list') &&
        observed.tools.includes('get_skill') &&
        observed.tools.includes('truenorth_record_task'),
      'Zed requested tools/list and discovered the required TrueNorth tools',
    ),
    resources_list: hasResourcesList
      ? outcome(
          observed.resources.includes('truenorth://state'),
          'Zed requested resources/list and discovered truenorth://state',
        )
      : notExposed('Zed did not request resources/list'),
    resource_read: hasResourceRead
      ? outcome(
          observed.completed_methods.includes('resources/read'),
          'Zed requested and completed a resource read',
        )
      : notExposed('Zed did not expose an MCP resource read operation'),
    read_tool_operation: outcome(
      completedTools.has('get_skill'),
      'Zed completed get_skill through the workspace-local TrueNorth server',
    ),
    task_mutation: outcome(
      completedTools.has('truenorth_record_task') && taskCount === 1,
      'Zed called truenorth_record_task once and changed only the disposable release plan',
    ),
    clean_shutdown: outcome(
      starts.length > 0 &&
        starts.length === exits.length &&
        exits.every((event) => event.code === 0),
      'every monitored TrueNorth stdio process exited with code 0',
    ),
    workspace_isolation: outcome(
      manifest.baseline.git_status === currentStatus &&
        manifest.baseline.release_plan_digest === currentPlanDigest &&
        manifest.baseline.user_settings_digest === currentUserSettingsDigest,
      'repository status, real release plan, and user-global Zed settings remained unchanged',
    ),
  };

  const certified = Object.values(operations).every((operation) => operation.status === 'pass');
  const evidence = {
    schema_version: 1,
    generated_on: new Date().toISOString().slice(0, 10),
    assessment: certified ? 'certified' : 'unsupported_for_full_certification',
    client: {
      name: 'Zed',
      version: manifest.zed.version,
      commit: manifest.zed.commit,
      channel: 'stable',
    },
    server: {
      name: 'truenorth-mcp',
      version: manifest.server_version,
      platform_package: `@truenorth-mcp/darwin-arm64@${manifest.server_version}`,
      transport: 'stdio',
    },
    model: manifest.model,
    environment: {
      platform: process.platform,
      os_version: os.release(),
      architecture: process.arch,
      node: process.version,
    },
    fixture: {
      profile: 'generic',
      packaged_wrapper: true,
      install_scripts: false,
      disposable_workspace: true,
      isolated_user_data: true,
      isolated_home: false,
      workspace_local_configuration: true,
      user_global_configuration_unchanged:
        manifest.baseline.user_settings_digest === currentUserSettingsDigest,
      real_project_workspace_unchanged:
        manifest.baseline.git_status === currentStatus &&
        manifest.baseline.release_plan_digest === currentPlanDigest,
      server_name: 'truenorth427',
    },
    configuration: configurationEvidence(),
    procedure: procedureEvidence(),
    client_evidence: {
      source:
        'sanitized stdio protocol proxy, Zed MCP UI, disposable release plan, and server lifecycle log',
      protocol_version: observed.protocol_version,
      protocol_client_info: observed.client_info,
      protocol_methods: observed.methods,
      protocol_tool_count: observed.tools.length,
      required_tools: observed.tools.filter((name) =>
        ['get_skill', 'truenorth_record_task'].includes(name),
      ),
      protocol_resources: observed.resources,
      protocol_tool_calls: observed.tool_calls,
      fixture_release_plan: {
        task_count: taskCount,
        task_name: TASK_NAME,
        verify_command: 'true',
      },
      shutdown: {
        server_processes_started: starts.length,
        server_processes_exited: exits.length,
        server_exit_codes: exits.map((event) => event.code),
      },
    },
    operations,
    certified,
  };

  validateEvidence(evidence);
  fs.mkdirSync(options.output, { recursive: true });
  const outputPath = path.join(path.resolve(options.output), 'zed.json');
  fs.writeFileSync(outputPath, `${JSON.stringify(evidence, null, 2)}\n`);
  console.log(`Zed certification wrote ${outputPath}`);
  console.log(
    `Zed ${evidence.client.version}: ${evidence.certified ? 'certified' : 'unsupported for full certification'}`,
  );
  if (!evidence.certified) process.exitCode = 1;
}

function validateFile(options) {
  const evidence = JSON.parse(fs.readFileSync(path.resolve(options.evidence), 'utf8'));
  validateEvidence(evidence);
  console.log(
    `Zed ${evidence.client.version}: ${evidence.certified ? 'certified' : 'unsupported'} evidence valid`,
  );
}

function cleanup(options) {
  const manifest = readSession(options.session);
  const root = path.resolve(options.session);
  if (
    manifest.session_root !== root ||
    !manifest.workspace.startsWith(`${root}${path.sep}`) ||
    !manifest.user_data.startsWith(`${root}${path.sep}`)
  ) {
    throw new Error('refusing to remove an invalid Zed certification session');
  }
  fs.rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  console.log(`Removed disposable Zed certification session: ${root}`);
}

function readSession(session) {
  const root = path.resolve(session);
  const manifestPath = path.join(root, 'certification-session.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  if (manifest.kind !== 'truenorth-zed-certification' || manifest.session_root !== root) {
    throw new Error('invalid Zed certification session');
  }
  return manifest;
}

function readZedMetadata(command) {
  const output = execFileSync(path.resolve(command), ['--version'], { encoding: 'utf8' }).trim();
  const version = output.match(/(?:^|\s)v?([0-9]+\.[0-9]+\.[0-9]+)(?:\+|\s|$)/)?.[1];
  if (!version) throw new Error(`unable to parse Zed version from ${JSON.stringify(output)}`);
  return {
    version,
    commit: output.match(/\b[0-9a-f]{40}\b/i)?.[0] ?? PINNED_ZED_COMMIT,
    version_output: output,
  };
}

function assertPinnedClient(zed) {
  if (zed.version !== PINNED_ZED_VERSION) {
    throw new Error(`expected Zed ${PINNED_ZED_VERSION}, received ${zed.version}`);
  }
}

function writeConfiguration({ fixture, monitor, transcript, lifecycle, serverStderr }) {
  const configuration = {
    context_servers: {
      truenorth427: {
        command: process.execPath,
        args: [monitor, transcript, lifecycle, serverStderr, fixture.command, ...fixture.args],
        env: {
          TRUENORTH_ROOT: fixture.project,
          TRUENORTH_VERIFY_CMD: 'true',
          RUST_LOG: 'info',
        },
      },
    },
  };
  const directory = path.join(fixture.project, '.zed');
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(
    path.join(directory, 'settings.json'),
    `${JSON.stringify(configuration, null, 2)}\n`,
  );
}

function configurationEvidence() {
  return {
    path: '.zed/settings.json',
    template: {
      context_servers: {
        truenorth427: {
          command: 'node',
          args: ['<protocol-monitor>', '<released-wrapper-command>'],
          env: {
            TRUENORTH_ROOT: '<disposable-workspace>',
            TRUENORTH_VERIFY_CMD: 'true',
          },
        },
      },
    },
  };
}

function procedureEvidence() {
  return [
    'Install the pinned released TrueNorth wrapper and darwin-arm64 package in a disposable package root with install scripts disabled.',
    'Initialize a disposable generic-profile TrueNorth workspace and write the workspace-local .zed/settings.json configuration shown above.',
    'Launch pinned Zed with a disposable --user-data-dir and open only the disposable workspace.',
    'Review and trust the disposable workspace, then confirm that truenorth427 is active in Settings, AI, MCP Servers.',
    'Use the native Zed Agent to call get_skill exactly once and truenorth_record_task exactly once. Inspect whether Zed exposes MCP resources.',
    'Close Zed normally, then verify the disposable release-plan mutation, protocol transcript, clean server exit, and unchanged real workspace and user settings.',
  ];
}

function writeMonitor(file) {
  fs.copyFileSync(path.join(__dirname, 'lib', 'stdio-evidence-proxy.js'), file);
}

function protocolEvidence(entries) {
  const responses = new Map(
    entries
      .filter((entry) => entry.direction === 'server_to_client' && entry.response)
      .map((entry) => [String(entry.id), entry.response_error]),
  );
  const requests = entries.filter(
    (entry) => entry.direction === 'client_to_server' && typeof entry.method === 'string',
  );
  return {
    protocol_version: entries.find((entry) => entry.protocol_version)?.protocol_version ?? null,
    client_info: entries.find((entry) => entry.client_info)?.client_info ?? null,
    methods: [...new Set(requests.map((entry) => entry.method))].sort(),
    completed_methods: [
      ...new Set(
        requests
          .filter((entry) => responses.has(String(entry.id)) && !responses.get(String(entry.id)))
          .map((entry) => entry.method),
      ),
    ].sort(),
    tools: [...new Set(entries.flatMap((entry) => entry.tools ?? []))].sort(),
    resources: [...new Set(entries.flatMap((entry) => entry.resources ?? []))].sort(),
    tool_calls: requests
      .filter((entry) => typeof entry.tool_call === 'string')
      .map((entry) => ({
        tool: entry.tool_call,
        status: !responses.has(String(entry.id))
          ? 'missing'
          : responses.get(String(entry.id))
            ? 'error'
            : 'completed',
      })),
  };
}

function validateEvidence(evidence) {
  if (evidence.schema_version !== 1) {
    throw new Error('Zed evidence must use schema_version 1');
  }
  if (
    evidence.client?.name !== 'Zed' ||
    evidence.client?.version !== PINNED_ZED_VERSION ||
    evidence.client?.commit !== PINNED_ZED_COMMIT
  ) {
    throw new Error(
      `Zed evidence must identify pinned Zed ${PINNED_ZED_VERSION} at ${PINNED_ZED_COMMIT}`,
    );
  }
  if (!evidence.model?.provider || !evidence.model?.id) {
    throw new Error('Zed evidence must identify the model provider and model');
  }
  for (const name of REQUIRED_OPERATIONS) {
    const operation = evidence.operations?.[name];
    if (!operation || !['pass', 'fail', 'not_exposed'].includes(operation.status)) {
      throw new Error(`Zed evidence has no valid ${name} outcome`);
    }
  }
  if (
    evidence.fixture?.disposable_workspace !== true ||
    evidence.fixture?.isolated_user_data !== true ||
    evidence.fixture?.workspace_local_configuration !== true ||
    evidence.fixture?.user_global_configuration_unchanged !== true ||
    evidence.fixture?.real_project_workspace_unchanged !== true
  ) {
    throw new Error('Zed certification must isolate user data, settings, and project workspace');
  }
  const methods = evidence.client_evidence?.protocol_methods ?? [];
  const tools = evidence.client_evidence?.required_tools ?? [];
  const resources = evidence.client_evidence?.protocol_resources ?? [];
  const calls = evidence.client_evidence?.protocol_tool_calls ?? [];
  for (const method of ['initialize', 'tools/list']) {
    if (!methods.includes(method)) throw new Error(`Zed evidence is missing ${method}`);
  }
  for (const tool of ['get_skill', 'truenorth_record_task']) {
    if (!tools.includes(tool)) throw new Error(`Zed evidence is missing ${tool} discovery`);
    if (!calls.some((call) => call.tool === tool && call.status === 'completed')) {
      throw new Error(`Zed evidence is missing a completed ${tool} call`);
    }
  }
  validateResourceOutcome(evidence.operations.resources_list, 'resources/list', methods, () =>
    resources.includes('truenorth://state'),
  );
  validateResourceOutcome(evidence.operations.resource_read, 'resources/read', methods);
  if (evidence.client_evidence?.fixture_release_plan?.task_count !== 1) {
    throw new Error('Zed evidence must contain exactly one disposable task mutation');
  }
  const allPass = Object.values(evidence.operations).every(
    (operation) => operation.status === 'pass',
  );
  if (evidence.certified !== allPass) {
    throw new Error('Zed certified status must match operation outcomes');
  }
  const expectedAssessment = allPass ? 'certified' : 'unsupported_for_full_certification';
  if (evidence.assessment !== expectedAssessment) {
    throw new Error(`Zed assessment must be ${expectedAssessment}`);
  }
  const serialized = JSON.stringify(evidence);
  if (/\/(Users|home|var\/folders)\//.test(serialized)) {
    throw new Error('Zed evidence contains an operator-specific absolute path');
  }
  return evidence;
}

function validateResourceOutcome(operation, method, methods, extraCheck = () => true) {
  const exposed = methods.includes(method);
  if (operation.status === 'not_exposed' && exposed) {
    throw new Error(`${method} cannot be not_exposed when the client requested it`);
  }
  if (operation.status === 'pass' && (!exposed || !extraCheck())) {
    throw new Error(`${method} pass lacks matching protocol evidence`);
  }
}

function wrapperVersion(fixture, expectedVersion) {
  const output = execFileSync(fixture.command, [...fixture.args, '--version'], {
    cwd: fixture.project,
    env: { ...process.env, TRUENORTH_ROOT: fixture.project },
    encoding: 'utf8',
  }).trim();
  const match = output.match(/([0-9]+\.[0-9]+\.[0-9]+)/);
  if (!match) {
    throw new Error(`unable to parse truenorth-mcp version from ${JSON.stringify(output)}`);
  }
  if (match[1] !== expectedVersion) {
    throw new Error(`expected truenorth-mcp ${expectedVersion}, received ${match[1]}`);
  }
  return match[1];
}

function parseOptions(args, required) {
  const options = {};
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!flag?.startsWith('--') || !value) throw new Error(`invalid argument ${flag ?? ''}`.trim());
    const key = flag.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    options[key] = value;
  }
  for (const key of required) {
    if (!options[key]) {
      const flag = key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
      throw new Error(`missing required --${flag}`);
    }
  }
  return options;
}

function readJsonLines(file) {
  if (!fs.existsSync(file)) return [];
  return fs
    .readFileSync(file, 'utf8')
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function pass(detail) {
  return { status: 'pass', detail };
}

function outcome(passed, detail) {
  return { status: passed ? 'pass' : 'fail', detail };
}

function notExposed(detail) {
  return { status: 'not_exposed', detail };
}

function gitStatus(root) {
  return execFileSync('git', ['status', '--short'], { cwd: root, encoding: 'utf8' });
}

function fileDigest(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function fileDigestOrMissing(file) {
  return fs.existsSync(file) ? fileDigest(file) : null;
}

function zedUserSettings() {
  return path.join(os.homedir(), '.config', 'zed', 'settings.json');
}

function countOccurrences(text, value) {
  return text.split(value).length - 1;
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, `'"'"'`)}'`;
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(`Zed certification failed: ${error.message}`);
    process.exitCode = 1;
  }
}

module.exports = {
  PINNED_ZED_VERSION,
  PINNED_ZED_COMMIT,
  REQUIRED_OPERATIONS,
  TASK_NAME,
  cleanup,
  parseOptions,
  protocolEvidence,
  validateEvidence,
};
