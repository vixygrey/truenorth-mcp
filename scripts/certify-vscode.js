'use strict';

const { execFileSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { createPackedWrapperFixture } = require('./lib/packed-artifact.js');

const PINNED_VSCODE_VERSION = '1.139.1';
const PINNED_COPILOT_VERSION = '0.67.0';
const TASK_NAME = 'Certify VS Code';
const REQUIRED_OPERATIONS = [
  'install',
  'initialize',
  'tools_list',
  'resources_list',
  'read_operation',
  'task_mutation',
  'clean_shutdown',
  'workspace_isolation',
];

function main() {
  const [command, ...args] = process.argv.slice(2);
  if (command === 'prepare')
    return prepare(parseOptions(args, ['expectedVersion', 'platformPackage', 'vscodeApp']));
  if (command === 'collect') return collect(parseOptions(args, ['session', 'output']));
  if (command === 'validate') return validateFile(parseOptions(args, ['evidence']));
  if (command === 'cleanup') return cleanup(parseOptions(args, ['session']));
  throw new Error('usage: certify-vscode.js <prepare|collect|validate|cleanup> [options]');
}

function prepare(options) {
  if (process.platform !== 'darwin' || process.arch !== 'arm64') {
    throw new Error('the pinned VS Code certificate requires darwin-arm64');
  }
  const repoRoot = path.resolve(__dirname, '..');
  const vscode = readVscodeMetadata(options.vscodeApp);
  assertPinnedClient(vscode);

  const fixture = createPackedWrapperFixture(options.platformPackage, 'tn-vscode-certification-');
  try {
    const userData = path.join(fixture.work, 'user-data');
    const extensions = path.join(fixture.work, 'extensions');
    const transcript = path.join(fixture.work, 'mcp-transcript.jsonl');
    const lifecycle = path.join(fixture.work, 'mcp-lifecycle.jsonl');
    const monitor = path.join(fixture.work, 'server-monitor.js');
    for (const directory of [userData, extensions]) fs.mkdirSync(directory, { recursive: true });
    writeMonitor(monitor);
    writeConfiguration({ fixture, monitor, transcript, lifecycle });

    const serverVersion = wrapperVersion(fixture, options.expectedVersion);
    const manifest = {
      kind: 'truenorth-vscode-certification',
      session_root: fixture.work,
      repository_root: repoRoot,
      workspace: fixture.project,
      user_data: userData,
      extensions,
      transcript,
      lifecycle,
      vscode,
      server_version: serverVersion,
      baseline: {
        git_status: gitStatus(repoRoot),
        release_plan_digest: fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml')),
      },
    };
    const manifestPath = path.join(fixture.work, 'certification-session.json');
    fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

    const executable = path.join(path.resolve(options.vscodeApp), 'Contents', 'MacOS', 'Code');
    if (!fs.existsSync(executable)) throw new Error('VS Code application executable is missing');
    console.log(`Prepared disposable certification session: ${fixture.work}`);
    console.log('Launch the isolated VS Code instance:');
    console.log(
      `${shellQuote(executable)} --user-data-dir ${shellQuote(userData)} --extensions-dir ${shellQuote(extensions)} --new-window ${shellQuote(fixture.project)}`,
    );
    console.log('After the manual steps and clean server stop, collect evidence:');
    console.log(
      `node scripts/certify-vscode.js collect --session ${shellQuote(fixture.work)} --output compatibility`,
    );
    console.log('Then remove the disposable session:');
    console.log(`node scripts/certify-vscode.js cleanup --session ${shellQuote(fixture.work)}`);
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
  const currentStatus = gitStatus(manifest.repository_root);
  const currentPlanDigest = fileDigest(
    path.join(manifest.repository_root, '.agent', 'tasks', 'release-plan.yml'),
  );
  const starts = lifecycle.filter((event) => event.event === 'start');
  const exits = lifecycle.filter((event) => event.event === 'exit');
  const completedTools = new Set(
    observed.tool_calls.filter((call) => call.status === 'completed').map((call) => call.tool),
  );

  const operations = {
    install: pass(
      `packed truenorth-mcp ${manifest.server_version} wrapper installed with scripts disabled`,
    ),
    initialize: outcome(
      observed.methods.includes('initialize') &&
        observed.client_info?.name === 'Visual Studio Code',
      'VS Code initialized the TrueNorth stdio server',
    ),
    tools_list: outcome(
      observed.methods.includes('tools/list') &&
        observed.tools.includes('get_skill') &&
        observed.tools.includes('truenorth_record_task'),
      'VS Code requested tools/list and discovered the required TrueNorth tools',
    ),
    resources_list: outcome(
      observed.methods.includes('resources/list') &&
        observed.resources.includes('truenorth://state'),
      'VS Code requested resources/list and discovered truenorth://state',
    ),
    read_operation: outcome(
      completedTools.has('get_skill') || observed.methods.includes('resources/read'),
      'VS Code completed a TrueNorth read operation',
    ),
    task_mutation: outcome(
      completedTools.has('truenorth_record_task') && plan.includes(TASK_NAME),
      'Copilot called truenorth_record_task and changed the disposable release plan',
    ),
    clean_shutdown: outcome(
      starts.length > 0 &&
        starts.length === exits.length &&
        exits.every((event) => event.code === 0),
      'every monitored TrueNorth stdio process exited with code 0',
    ),
    workspace_isolation: outcome(
      manifest.baseline.git_status === currentStatus &&
        manifest.baseline.release_plan_digest === currentPlanDigest,
      'repository status and release plan were unchanged during certification',
    ),
  };

  const evidence = {
    schema_version: 1,
    generated_on: new Date().toISOString().slice(0, 10),
    client: {
      name: 'Visual Studio Code',
      version: manifest.vscode.version,
      commit: manifest.vscode.commit,
      extension: {
        name: 'GitHub Copilot Chat',
        version: manifest.vscode.copilot_version,
      },
    },
    server: {
      name: 'truenorth-mcp',
      version: manifest.server_version,
      transport: 'stdio',
    },
    environment: {
      platform: process.platform,
      architecture: process.arch,
    },
    fixture: {
      profile: 'generic',
      packaged_wrapper: true,
      install_scripts: false,
      disposable_workspace: true,
      disposable_vscode_profile: true,
      isolated_home: false,
      workspace_local_configuration: true,
      normal_vscode_profile_unchanged: true,
      real_project_workspace_unchanged: true,
      host_authentication_provider_reused: true,
    },
    client_evidence: {
      protocol_version: observed.protocol_version,
      client_info: observed.client_info,
      protocol_methods: observed.methods,
      protocol_tool_count: observed.tools.length,
      required_tools: observed.tools.filter((name) =>
        ['get_skill', 'truenorth_record_task'].includes(name),
      ),
      protocol_resources: observed.resources,
      protocol_tool_calls: observed.tool_calls,
      server_processes: { started: starts.length, exited: exits.length },
      server_exit_codes: exits.map((event) => event.code),
    },
    operations,
    warnings: [],
    certified: Object.values(operations).every((operation) => operation.status === 'pass'),
  };
  validateEvidence(evidence);
  fs.mkdirSync(options.output, { recursive: true });
  const outputPath = path.join(path.resolve(options.output), 'vscode-copilot.json');
  fs.writeFileSync(outputPath, `${JSON.stringify(evidence, null, 2)}\n`);
  console.log(`VS Code certification wrote ${outputPath}`);
  console.log(
    `VS Code ${evidence.client.version} with Copilot Chat ${evidence.client.extension.version}: ${evidence.certified ? 'certified' : 'failed'}`,
  );
  if (!evidence.certified) process.exitCode = 1;
}

function validateFile(options) {
  const evidence = JSON.parse(fs.readFileSync(path.resolve(options.evidence), 'utf8'));
  validateEvidence(evidence);
  console.log(
    `VS Code ${evidence.client.version} with Copilot Chat ${evidence.client.extension.version}: certified evidence valid`,
  );
}

function cleanup(options) {
  const manifest = readSession(options.session);
  const root = path.resolve(options.session);
  if (manifest.session_root !== root || !manifest.workspace.startsWith(`${root}${path.sep}`)) {
    throw new Error('refusing to remove an invalid certification session');
  }
  fs.rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  console.log(`Removed disposable certification session: ${root}`);
}

function readSession(session) {
  const root = path.resolve(session);
  const manifestPath = path.join(root, 'certification-session.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  if (manifest.kind !== 'truenorth-vscode-certification' || manifest.session_root !== root) {
    throw new Error('invalid VS Code certification session');
  }
  return manifest;
}

function readVscodeMetadata(app) {
  const root = path.resolve(app);
  const resources = path.join(root, 'Contents', 'Resources', 'app');
  const packageMetadata = JSON.parse(fs.readFileSync(path.join(resources, 'package.json'), 'utf8'));
  const productMetadata = JSON.parse(fs.readFileSync(path.join(resources, 'product.json'), 'utf8'));
  const copilotMetadata = JSON.parse(
    fs.readFileSync(path.join(resources, 'extensions', 'copilot', 'package.json'), 'utf8'),
  );
  return {
    version: packageMetadata.version,
    commit: productMetadata.commit,
    copilot_version: copilotMetadata.version,
  };
}

function assertPinnedClient(vscode) {
  if (vscode.version !== PINNED_VSCODE_VERSION) {
    throw new Error(`expected VS Code ${PINNED_VSCODE_VERSION}, received ${vscode.version}`);
  }
  if (vscode.copilot_version !== PINNED_COPILOT_VERSION) {
    throw new Error(
      `expected GitHub Copilot Chat ${PINNED_COPILOT_VERSION}, received ${vscode.copilot_version}`,
    );
  }
  if (!vscode.commit) throw new Error('VS Code product metadata has no commit');
}

function writeConfiguration({ fixture, monitor, transcript, lifecycle }) {
  const configuration = {
    servers: {
      truenorth: {
        type: 'stdio',
        command: process.execPath,
        args: [monitor, transcript, lifecycle, fixture.command, ...fixture.args],
        env: {
          TRUENORTH_ROOT: fixture.project,
          TRUENORTH_VERIFY_CMD: 'true',
          RUST_LOG: 'info',
        },
      },
    },
  };
  const directory = path.join(fixture.project, '.vscode');
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(path.join(directory, 'mcp.json'), `${JSON.stringify(configuration, null, 2)}\n`);
}

function writeMonitor(file) {
  fs.writeFileSync(
    file,
    `'use strict';\nconst fs = require('node:fs');\nconst { spawn } = require('node:child_process');\nconst [transcript, lifecycle, command, ...args] = process.argv.slice(2);\nconst child = spawn(command, args, { stdio: ['pipe', 'pipe', 'inherit'], env: process.env });\nconst append = (target, value) => fs.appendFileSync(target, JSON.stringify(value) + '\\n');\nappend(lifecycle, { event: 'start', pid: child.pid });\nfunction forward(source, target, direction) {\n  let pending = '';\n  source.on('data', (chunk) => {\n    target.write(chunk);\n    pending += chunk.toString('utf8');\n    for (;;) {\n      const index = pending.indexOf('\\n');\n      if (index < 0) break;\n      const line = pending.slice(0, index).trim();\n      pending = pending.slice(index + 1);\n      if (!line) continue;\n      try {\n        const frame = JSON.parse(line);\n        append(transcript, {\n          direction,\n          method: frame.method || null,\n          id: frame.id ?? null,\n          protocol_version: frame.params?.protocolVersion || frame.result?.protocolVersion || null,\n          client_info: frame.params?.clientInfo || null,\n          tools: Array.isArray(frame.result?.tools) ? frame.result.tools.map((tool) => tool.name).filter(Boolean) : [],\n          resources: Array.isArray(frame.result?.resources) ? frame.result.resources.map((resource) => resource.uri).filter(Boolean) : [],\n          tool_call: frame.method === 'tools/call' && typeof frame.params?.name === 'string' ? frame.params.name : null,\n          tool_response: direction === 'server_to_client' && frame.id != null && ('result' in frame || 'error' in frame),\n          tool_error: Boolean(frame.error || frame.result?.isError),\n        });\n      } catch {}\n    }\n  });\n}\nforward(process.stdin, child.stdin, 'client_to_server');\nforward(child.stdout, process.stdout, 'server_to_client');\nprocess.stdin.on('end', () => child.stdin.end());\nchild.on('exit', (code, signal) => {\n  append(lifecycle, { event: 'exit', pid: child.pid, code, signal });\n  process.exitCode = code ?? 1;\n});\nfor (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => child.kill(signal));\n`,
  );
}

function protocolEvidence(entries) {
  const responses = new Map(
    entries
      .filter((entry) => entry.direction === 'server_to_client' && entry.tool_response)
      .map((entry) => [String(entry.id), entry.tool_error]),
  );
  return {
    protocol_version: entries.find((entry) => entry.protocol_version)?.protocol_version ?? null,
    client_info: entries.find((entry) => entry.client_info)?.client_info ?? null,
    methods: [
      ...new Set(
        entries
          .filter(
            (entry) => entry.direction === 'client_to_server' && typeof entry.method === 'string',
          )
          .map((entry) => entry.method),
      ),
    ].sort(),
    tools: [...new Set(entries.flatMap((entry) => entry.tools ?? []))].sort(),
    resources: [...new Set(entries.flatMap((entry) => entry.resources ?? []))].sort(),
    tool_calls: entries
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
    throw new Error('certification evidence must use schema_version 1');
  }
  if (
    evidence.client?.name !== 'Visual Studio Code' ||
    evidence.client?.version !== PINNED_VSCODE_VERSION ||
    evidence.client?.extension?.version !== PINNED_COPILOT_VERSION
  ) {
    throw new Error('certification evidence does not identify the pinned VS Code client pair');
  }
  for (const name of REQUIRED_OPERATIONS) {
    const operation = evidence.operations?.[name];
    if (!operation || !['pass', 'fail', 'not_exposed'].includes(operation.status)) {
      throw new Error(`certification evidence has no valid ${name} outcome`);
    }
  }
  if (
    evidence.fixture?.disposable_workspace !== true ||
    evidence.fixture?.disposable_vscode_profile !== true ||
    evidence.fixture?.workspace_local_configuration !== true ||
    evidence.fixture?.normal_vscode_profile_unchanged !== true ||
    evidence.fixture?.real_project_workspace_unchanged !== true
  ) {
    throw new Error('VS Code certification must isolate the profile and project workspace');
  }
  const methods = evidence.client_evidence?.protocol_methods ?? [];
  const tools = evidence.client_evidence?.required_tools ?? [];
  const resources = evidence.client_evidence?.protocol_resources ?? [];
  const calls = evidence.client_evidence?.protocol_tool_calls ?? [];
  for (const method of ['initialize', 'tools/list', 'resources/list']) {
    if (!methods.includes(method)) throw new Error(`client evidence is missing ${method}`);
  }
  for (const tool of ['get_skill', 'truenorth_record_task']) {
    if (!tools.includes(tool)) throw new Error(`client evidence is missing ${tool} discovery`);
    if (!calls.some((call) => call.tool === tool && call.status === 'completed')) {
      throw new Error(`client evidence is missing a completed ${tool} call`);
    }
  }
  if (!resources.includes('truenorth://state')) {
    throw new Error('client evidence is missing truenorth://state discovery');
  }
  const allPass = Object.values(evidence.operations).every(
    (operation) => operation.status === 'pass',
  );
  if (evidence.certified !== allPass) throw new Error('certified must match operation outcomes');
  const serialized = JSON.stringify(evidence);
  if (/\/(Users|home|var\/folders)\//.test(serialized)) {
    throw new Error('certification evidence contains an operator-specific absolute path');
  }
  return evidence;
}

function wrapperVersion(fixture, expectedVersion) {
  const output = execFileSync(fixture.command, [...fixture.args, '--version'], {
    cwd: fixture.project,
    env: { ...process.env, TRUENORTH_ROOT: fixture.project },
    encoding: 'utf8',
  }).trim();
  const match = output.match(/([0-9]+\.[0-9]+\.[0-9]+)/);
  if (!match)
    throw new Error(`unable to parse truenorth-mcp version from ${JSON.stringify(output)}`);
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

function gitStatus(root) {
  return execFileSync('git', ['status', '--short'], { cwd: root, encoding: 'utf8' });
}

function fileDigest(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, `'"'"'`)}'`;
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(`VS Code certification failed: ${error.message}`);
    process.exitCode = 1;
  }
}

module.exports = {
  PINNED_COPILOT_VERSION,
  PINNED_VSCODE_VERSION,
  REQUIRED_OPERATIONS,
  parseOptions,
  protocolEvidence,
  validateEvidence,
};
