'use strict';

const { execFileSync, spawn, spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createPackedWrapperFixture } = require('./lib/packed-artifact.js');

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
const TASK_NAME = 'Certify OpenCode';

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const repoRoot = path.resolve(__dirname, '..');
  const beforeStatus = gitStatus(repoRoot);
  const beforePlan = fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml'));
  const fixture = createPackedWrapperFixture(options.platformPackage, 'tn-opencode-certification-');
  const home = path.join(fixture.work, 'home');
  const transcript = path.join(fixture.work, 'mcp-transcript.jsonl');
  const lifecycle = path.join(fixture.work, 'mcp-lifecycle.jsonl');
  const monitor = path.join(fixture.work, 'server-monitor.js');
  let clientServer;

  try {
    preflightModel(options.baseUrl, options.modelId);
    fs.mkdirSync(home, { recursive: true });
    writeMonitor(monitor);
    const config = writeConfiguration({ fixture, home, monitor, transcript, lifecycle, options });
    const env = isolatedEnvironment(home, config);
    const clientVersion = opencodeVersion(options.opencode, env);

    const readServerTool = 'get_skill';
    const mutationServerTool = 'truenorth_record_task';

    const standaloneResult = invokeOpenCode(
      options,
      fixture.project,
      env,
      ['api', '--standalone', 'config.get', '--param', `directory=${fixture.project}`],
      'standalone configuration probe',
    );

    clientServer = await startOpenCodeServer(options, fixture.project, env);
    const configResult = invokeOpenCode(
      options,
      fixture.project,
      env,
      [
        'api',
        '--server',
        clientServer.url,
        'config.get',
        '--param',
        `directory=${fixture.project}`,
      ],
      'configuration load',
    );
    const connectResult = invokeOpenCode(
      options,
      fixture.project,
      env,
      [
        'api',
        '--server',
        clientServer.url,
        'experimental.mcp.connect',
        '--param',
        'server=truenorth',
        '--param',
        `directory=${fixture.project}`,
      ],
      'MCP connect',
    );
    const reloadResult = invokeOpenCode(
      options,
      fixture.project,
      env,
      ['reload', '--server', clientServer.url],
      'tool catalog reload',
    );
    const reconnectResult = invokeOpenCode(
      options,
      fixture.project,
      env,
      [
        'api',
        '--server',
        clientServer.url,
        'experimental.mcp.connect',
        '--param',
        'server=truenorth',
        '--param',
        `directory=${fixture.project}`,
      ],
      'MCP reconnect',
    );
    const resourceResult = invokeOpenCode(
      options,
      fixture.project,
      env,
      [
        'api',
        '--server',
        clientServer.url,
        'mcp.resource.catalog',
        '--param',
        `directory=${fixture.project}`,
      ],
      'resources/list',
    );

    const operationResult = invokeOpenCode(
      options,
      fixture.project,
      env,
      [
        'run',
        '--server',
        clientServer.url,
        '--format',
        'json',
        '--auto',
        '--model',
        `llama.cpp/${options.modelId}`,
        `Use the execute tool exactly once with this exact code: const read = await tools.truenorth.get_skill({ name: "using-truenorth", tier: "lean" }); const mutation = await tools.truenorth.truenorth_record_task({ task_name: ${JSON.stringify(TASK_NAME)}, verify_command: "true" }); return { read, mutation }; Do not change or split the code. Stop after the result.`,
      ],
      'read and task mutation',
    );
    const clientServerExit = await stopOpenCodeServer(clientServer);
    clientServer = undefined;

    const protocol = readJsonLines(transcript);
    const lifecycleEvents = readJsonLines(lifecycle);
    const operationEvents = parseEventLines(operationResult.stdout);
    const resourceCatalog = parseJsonOutput(resourceResult.stdout);
    const catalogResources = resourceUris(resourceCatalog);
    const disposablePlanPath = path.join(fixture.project, '.agent', 'tasks', 'release-plan.yml');
    const disposablePlan = fs.existsSync(disposablePlanPath)
      ? fs.readFileSync(disposablePlanPath, 'utf8')
      : '';
    const afterStatus = gitStatus(repoRoot);
    const afterPlan = fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml'));

    const {
      methods,
      tools,
      resources: protocolResources,
      tool_calls: protocolToolCalls,
    } = protocolEvidence(protocol);
    const operationTool = completedTool(operationEvents, 'execute');
    const readProtocolCall = protocolToolCalls.some(
      (call) => call.tool === readServerTool && call.status === 'completed',
    );
    const mutationProtocolCall = protocolToolCalls.some(
      (call) => call.tool === mutationServerTool && call.status === 'completed',
    );
    const started = lifecycleEvents.filter((event) => event.event === 'start');
    const exited = lifecycleEvents.filter((event) => event.event === 'exit');
    const operations = {
      install: pass('packed wrapper installed with scripts disabled'),
      initialize: outcome(
        methods.includes('initialize'),
        'OpenCode sent initialize to the TrueNorth stdio server',
      ),
      tools_list: outcome(
        methods.includes('tools/list') &&
          tools.includes(readServerTool) &&
          tools.includes(mutationServerTool),
        'OpenCode sent tools/list and exposed the required TrueNorth tools',
      ),
      resources_list: outcome(
        methods.includes('resources/list') &&
          protocolResources.includes('truenorth://state') &&
          catalogResources.includes('truenorth://state'),
        'OpenCode sent resources/list and returned truenorth://state through its resource catalog API',
      ),
      read_operation: outcome(
        Boolean(operationTool) && readProtocolCall,
        'OpenCode JSON events and MCP protocol records show a completed get_skill call',
      ),
      task_mutation: outcome(
        Boolean(operationTool) && mutationProtocolCall && disposablePlan.includes(TASK_NAME),
        'OpenCode JSON events and MCP protocol records show record_task changed the disposable release plan',
      ),
      clean_shutdown: outcome(
        started.length >= 1 &&
          exited.length === started.length &&
          exited.every((event) => event.code === 0) &&
          ([0, 130].includes(clientServerExit.code) || clientServerExit.signal === 'SIGTERM'),
        'every OpenCode flow closed its monitored TrueNorth stdio child',
      ),
      workspace_isolation: outcome(
        beforeStatus === afterStatus && beforePlan === afterPlan,
        'repository status and release plan were unchanged during certification',
      ),
    };

    const evidence = {
      schema_version: 1,
      generated_on: new Date().toISOString().slice(0, 10),
      client: { name: 'OpenCode', version: clientVersion },
      server: { name: 'truenorth-mcp', version: options.expectedVersion, transport: 'stdio' },
      model: {
        provider: 'llama.cpp',
        id: options.modelId,
        endpoint: options.baseUrl,
        authentication: 'none',
        hosted_fallback: false,
      },
      environment: {
        platform: process.platform,
        architecture: process.arch,
        node: process.version,
      },
      fixture: {
        profile: 'generic',
        packaged_wrapper: true,
        install_scripts: false,
        disposable_workspace: true,
        isolated_home: true,
        standalone_configuration_probe: true,
        ephemeral_api_server: true,
        shared_service: false,
        project_local_configuration: true,
      },
      client_evidence: {
        protocol_methods: methods,
        protocol_tools: tools,
        protocol_resources: protocolResources,
        protocol_tool_calls: protocolToolCalls,
        resources: catalogResources,
        tool_events: operationEvents
          .filter((event) => event.type === 'tool_use')
          .map(summarizeTool),
        invocations: [
          standaloneResult,
          configResult,
          connectResult,
          reloadResult,
          reconnectResult,
          resourceResult,
          operationResult,
        ].map((result) => ({
          purpose: result.purpose,
          exit_code: result.status,
          signal: result.signal,
        })),
        api_server_exit: clientServerExit,
        server_processes: { started: started.length, exited: exited.length },
      },
      operations,
      certified: Object.values(operations).every((operation) => operation.status === 'pass'),
    };

    validateEvidence(evidence);
    fs.mkdirSync(options.output, { recursive: true });
    const outputPath = path.join(options.output, 'opencode.json');
    fs.writeFileSync(outputPath, `${JSON.stringify(evidence, null, 2)}\n`);
    console.log(`OpenCode certification wrote ${outputPath}`);
    console.log(`OpenCode ${clientVersion}: ${evidence.certified ? 'certified' : 'failed'}`);
    if (!evidence.certified) process.exitCode = 1;
  } finally {
    if (clientServer) await stopOpenCodeServer(clientServer).catch(() => {});
    fixture.cleanup();
  }
}

function startOpenCodeServer(options, cwd, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(options.opencode, ['serve', '--hostname', '127.0.0.1', '--port', '0'], {
      cwd,
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let diagnostic = '';
    let settled = false;
    const timer = setTimeout(
      () => {
        if (settled) return;
        settled = true;
        child.kill('SIGTERM');
        reject(new Error('OpenCode API server did not become ready'));
      },
      Math.min(options.timeoutMs, 30_000),
    );

    const observe = (chunk) => {
      if (settled) return;
      diagnostic += chunk.toString('utf8');
      const match = diagnostic.match(/server listening on (http:\/\/\S+)/);
      if (!match) return;
      settled = true;
      clearTimeout(timer);
      resolve({ child, url: match[1] });
    };
    child.stdout.on('data', observe);
    child.stderr.on('data', observe);
    child.once('error', (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(new Error(`OpenCode API server failed to start: ${error.message}`));
    });
    child.once('exit', (code, signal) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(
        new Error(
          `OpenCode API server exited before readiness (${code ?? signal}): ${scrubDiagnostic(diagnostic)}`,
        ),
      );
    });
  });
}

function stopOpenCodeServer(server, timeoutMs = 10_000) {
  const { child } = server;
  if (child.exitCode !== null || child.signalCode !== null) {
    return Promise.resolve({ code: child.exitCode, signal: child.signalCode });
  }
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill('SIGKILL');
      reject(new Error('OpenCode API server did not stop after SIGTERM'));
    }, timeoutMs);
    child.once('exit', (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal });
    });
    child.kill('SIGTERM');
  });
}

function writeConfiguration({ fixture, home, monitor, transcript, lifecycle, options }) {
  const config = {
    $schema: 'https://opencode.ai/config.json',
    model: `llama.cpp/${options.modelId}`,
    share: 'disabled',
    autoupdate: false,
    provider: {
      'llama.cpp': {
        npm: '@ai-sdk/openai-compatible',
        name: 'Local llama.cpp',
        options: { baseURL: `${options.baseUrl.replace(/\/$/, '')}/v1` },
        models: {
          [options.modelId]: {
            name: options.modelId,
            limit: { context: 32768, output: 4096 },
          },
        },
      },
    },
    mcp: {
      truenorth: {
        type: 'local',
        command: [
          process.execPath,
          monitor,
          transcript,
          lifecycle,
          fixture.command,
          ...fixture.args,
        ],
        environment: {
          TRUENORTH_ROOT: fixture.project,
          TRUENORTH_VERIFY_CMD: 'true',
          RUST_LOG: 'info',
        },
        enabled: true,
        timeout: 30000,
      },
    },
    permission: {
      '*': 'allow',
    },
  };
  const configPath = path.join(fixture.project, 'opencode.json');
  fs.writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  fs.mkdirSync(home, { recursive: true });
  return configPath;
}

function writeMonitor(file) {
  fs.writeFileSync(
    file,
    `'use strict';\nconst fs = require('node:fs');\nconst { spawn } = require('node:child_process');\nconst [transcript, lifecycle, command, ...args] = process.argv.slice(2);\nconst child = spawn(command, args, { stdio: ['pipe', 'pipe', 'inherit'], env: process.env });\nconst append = (file, value) => fs.appendFileSync(file, JSON.stringify(value) + '\\n');\nappend(lifecycle, { event: 'start', pid: child.pid });\nfunction forward(source, target, direction) {\n  let pending = '';\n  source.on('data', (chunk) => {\n    target.write(chunk);\n    pending += chunk.toString('utf8');\n    for (;;) {\n      const index = pending.indexOf('\\n');\n      if (index < 0) break;\n      const line = pending.slice(0, index).trim();\n      pending = pending.slice(index + 1);\n      if (!line) continue;\n      try {\n        const frame = JSON.parse(line);\n        append(transcript, {\n          direction,\n          method: frame.method || null,\n          id: frame.id ?? null,\n          tools: Array.isArray(frame.result?.tools) ? frame.result.tools.map((tool) => tool.name).filter(Boolean) : [],\n          resources: Array.isArray(frame.result?.resources) ? frame.result.resources.map((resource) => resource.uri).filter(Boolean) : [],\n          tool_call: frame.method === 'tools/call' && typeof frame.params?.name === 'string' ? frame.params.name : null,\n          tool_response: direction === 'server_to_client' && frame.id != null && ('result' in frame || 'error' in frame),\n          tool_error: Boolean(frame.error || frame.result?.isError),\n        });\n      } catch {}\n    }\n  });\n}\nforward(process.stdin, child.stdin, 'client_to_server');\nforward(child.stdout, process.stdout, 'server_to_client');\nprocess.stdin.on('end', () => child.stdin.end());\nchild.on('exit', (code, signal) => {\n  append(lifecycle, { event: 'exit', pid: child.pid, code, signal });\n  process.exitCode = code ?? 1;\n});\nfor (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => child.kill(signal));\n`,
  );
}

function isolatedEnvironment(home, config) {
  const env = {
    HOME: home,
    PATH: process.env.PATH,
    TMPDIR: process.env.TMPDIR || os.tmpdir(),
    LANG: process.env.LANG || 'C.UTF-8',
    XDG_CONFIG_HOME: path.join(home, 'config'),
    XDG_DATA_HOME: path.join(home, 'data'),
    XDG_CACHE_HOME: path.join(home, 'cache'),
    XDG_STATE_HOME: path.join(home, 'state'),
    OPENCODE_CONFIG: config,
    OPENCODE_DISABLE_AUTOUPDATE: '1',
    OPENCODE_SERVER_USERNAME: 'opencode',
    OPENCODE_SERVER_PASSWORD: crypto.randomBytes(32).toString('base64url'),
    NO_COLOR: '1',
  };
  if (process.env.LC_ALL) env.LC_ALL = process.env.LC_ALL;
  return env;
}

function invokeOpenCode(options, cwd, env, args, purpose) {
  const result = spawnSync(options.opencode, args, {
    cwd,
    env,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
    timeout: options.timeoutMs,
  });
  if (result.error) throw new Error(`OpenCode ${purpose} failed to run: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(
      `OpenCode ${purpose} exited ${result.status}: ${scrubDiagnostic(result.stderr || result.stdout)}`,
    );
  }
  return { purpose, status: result.status, signal: result.signal, stdout: result.stdout };
}

function parseEventLines(output) {
  return output
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}
function parseJsonOutput(output) {
  const trimmed = output.trim();
  if (!trimmed) return null;
  return JSON.parse(trimmed);
}

function resourceUris(value) {
  if (!value || typeof value !== 'object') return [];
  if (typeof value.uri === 'string') return [value.uri];
  return Object.values(value).flatMap(resourceUris);
}

function completedTool(events, name) {
  return events.find(
    (event) =>
      event.type === 'tool_use' &&
      event.part?.tool === name &&
      event.part?.state?.status === 'completed',
  );
}

function summarizeTool(event) {
  if (!event) return null;
  const calls = event.part.state.metadata?.metadata?.toolCalls;
  return {
    tool: event.part.tool,
    status: event.part.state.status,
    call_id: event.part.callID ?? event.part.id ?? null,
    calls: Array.isArray(calls)
      ? calls.map((call) => ({ tool: call.tool, status: call.status }))
      : [],
  };
}

function protocolEvidence(entries) {
  const responses = new Map(
    entries
      .filter((entry) => entry.direction === 'server_to_client' && entry.tool_response)
      .map((entry) => [String(entry.id), entry.tool_error]),
  );
  return {
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

function readJsonLines(file) {
  if (!fs.existsSync(file)) return [];
  return fs
    .readFileSync(file, 'utf8')
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function preflightModel(baseUrl, modelId) {
  const endpoint = `${baseUrl.replace(/\/$/, '')}/v1/models`;
  const result = spawnSync('xh', ['--check-status', 'GET', endpoint], {
    encoding: 'utf8',
    timeout: 10000,
  });
  if (result.status !== 0) throw new Error(`local model endpoint is unavailable at ${endpoint}`);
  const body = JSON.parse(result.stdout);
  if (!body.data?.some((model) => model.id === modelId)) {
    throw new Error(`local model endpoint does not expose ${modelId}`);
  }
}

function opencodeVersion(executable, env) {
  const output = execFileSync(executable, ['--version'], { env, encoding: 'utf8' }).trim();
  const match = output.match(/(?:opencode\s+)?v?([0-9]+\.[0-9]+\.[0-9]+)/i);
  if (!match) throw new Error(`unable to parse OpenCode version from ${JSON.stringify(output)}`);
  return match[1];
}

function parseArgs(args) {
  const options = {
    opencode: 'opencode',
    baseUrl: 'http://127.0.0.1:18081',
    modelId: 'hermes-3-llama-3.1-8b',
    timeoutMs: 240000,
  };
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!value) throw new Error(`missing value for ${flag}`);
    if (flag === '--expected-version') options.expectedVersion = value;
    else if (flag === '--platform-package') options.platformPackage = value;
    else if (flag === '--output') options.output = value;
    else if (flag === '--opencode') options.opencode = value;
    else if (flag === '--base-url') options.baseUrl = value;
    else if (flag === '--model-id') options.modelId = value;
    else if (flag === '--timeout-ms') options.timeoutMs = Number(value);
    else throw new Error(`unknown argument ${flag}`);
  }
  for (const key of ['expectedVersion', 'platformPackage', 'output']) {
    if (!options[key])
      throw new Error(
        `missing required --${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`,
      );
  }
  if (!Number.isFinite(options.timeoutMs) || options.timeoutMs <= 0)
    throw new Error('--timeout-ms must be positive');
  return options;
}

function validateEvidence(evidence) {
  if (evidence.schema_version !== 1)
    throw new Error('certification evidence must use schema_version 1');
  for (const name of REQUIRED_OPERATIONS) {
    const operation = evidence.operations?.[name];
    if (!operation || !['pass', 'fail', 'not_exposed'].includes(operation.status)) {
      throw new Error(`certification evidence has no valid ${name} outcome`);
    }
  }
  if (evidence.model?.authentication !== 'none' || evidence.model?.hosted_fallback !== false) {
    throw new Error(
      'OpenCode certification must use a keyless local model without hosted fallback',
    );
  }
  const allPass = Object.values(evidence.operations).every(
    (operation) => operation.status === 'pass',
  );
  if (evidence.certified !== allPass) throw new Error('certified must match operation outcomes');
  return evidence;
}

function pass(detail) {
  return { status: 'pass', detail };
}

function outcome(passed, detail) {
  return { status: passed ? 'pass' : 'fail', detail };
}

function scrubDiagnostic(value) {
  return String(value)
    .replace(/[\r\n]+/g, ' ')
    .slice(0, 1000);
}

function gitStatus(root) {
  return execFileSync('git', ['status', '--short'], { cwd: root, encoding: 'utf8' });
}

function fileDigest(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.stack ?? error.message);
    process.exitCode = 1;
  });
}

module.exports = {
  REQUIRED_OPERATIONS,
  completedTool,
  parseArgs,
  protocolEvidence,
  resourceUris,
  validateEvidence,
};
