'use strict';

const { execFileSync, spawn, spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const readline = require('node:readline');
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
const PROFILE = 'truenorth-certification';
const TASK_NAME = 'Certify Oh My Pi';

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const repoRoot = path.resolve(__dirname, '..');
  const beforeStatus = gitStatus(repoRoot);
  const beforePlan = fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml'));
  const fixture = createPackedWrapperFixture(options.platformPackage, 'tn-omp-certification-');
  const home = path.join(fixture.work, 'home');
  const sessions = path.join(fixture.work, 'sessions');
  const config = path.join(fixture.work, 'omp-config.json');
  const serverStart = path.join(fixture.work, 'server-start.json');
  const serverExit = path.join(fixture.work, 'server-exit.json');
  const monitor = path.join(fixture.work, 'server-monitor.js');
  let rpc;

  try {
    preflightModel(options.baseUrl, options.modelId);
    fs.mkdirSync(home, { recursive: true });
    fs.mkdirSync(sessions, { recursive: true });
    writeMonitor(monitor);
    writeOmpConfiguration({ config, fixture, home, monitor, serverStart, serverExit, options });

    const env = isolatedEnvironment(home, options.baseUrl);
    const ompVersion = clientVersion(options.omp, env);
    rpc = startRpc(options, fixture, config, sessions, env);
    console.error('OMP certification: waiting for RPC readiness');

    const ready = await rpc.waitFor((frame) => frame.type === 'ready', options.timeoutMs);
    console.error('OMP certification: RPC ready');
    await rpc.request({ type: 'negotiate_protocol', protocolVersion: 2 });
    const state = await rpc.request({ type: 'get_state' });
    console.error(`OMP certification: ${state.data?.dumpTools?.length ?? 0} tools discovered`);
    const tools = state.data?.dumpTools ?? [];
    const readTool = findTool(tools, '_get_skill');
    const mutationTool = findTool(tools, '_truenorth_record_task');

    const listResult = await runLocalCommand(rpc, '/mcp list', options.timeoutMs);
    const resourceResult = await runLocalCommand(rpc, '/mcp resources', options.timeoutMs);
    console.error('OMP certification: MCP list and resources observed');
    if (!readTool || !mutationTool) {
      throw new Error('OMP did not expose the required TrueNorth MCP tools');
    }

    const readFrames = await runAgentTurn(
      rpc,
      `Call ${readTool} with name using-truenorth and tier lean. Do not call any other tool. Stop after the tool result returns.`,
      options.timeoutMs,
    );
    console.error('OMP certification: read operation turn completed');
    const mutationFrames = await runAgentTurn(
      rpc,
      `Call ${mutationTool} with task_name ${JSON.stringify(TASK_NAME)} and verify_command ${JSON.stringify('true')}. Do not call any other tool. Stop after the tool result returns.`,
      options.timeoutMs,
    );
    console.error('OMP certification: mutation operation turn completed');
    const promptFrames = [...readFrames, ...mutationFrames];

    rpc.closeInput();
    const closed = await rpc.waitForExit(options.timeoutMs);
    console.error('OMP certification: OMP exited after stdin close');
    rpc = null;
    const serverLifecycle = await readServerLifecycle(serverStart, serverExit, options.timeoutMs);
    const disposablePlan = path.join(fixture.project, '.agent', 'tasks', 'release-plan.yml');
    const plan = fs.existsSync(disposablePlan) ? fs.readFileSync(disposablePlan, 'utf8') : '';
    const afterStatus = gitStatus(repoRoot);
    const afterPlan = fileDigest(path.join(repoRoot, '.agent', 'tasks', 'release-plan.yml'));

    const readEvents = toolEvents(promptFrames, readTool);
    const mutationEvents = toolEvents(promptFrames, mutationTool);
    const operations = {
      install: pass('packed wrapper installed with scripts disabled'),
      initialize: outcome(
        ready.supportedProtocolVersions?.includes(2) && state.data?.model?.provider === 'llama.cpp',
        'OMP RPC v2 started with the pinned keyless llama.cpp model and loaded MCP state',
      ),
      tools_list: outcome(
        Boolean(readTool && mutationTool),
        'OMP get_state dumpTools includes TrueNorth read and mutation tools',
      ),
      resources_list: outcome(
        resourceResult.output.includes('truenorth://state'),
        'OMP /mcp resources output includes truenorth://state',
      ),
      read_operation: outcome(
        readEvents.started && readEvents.ended && !readEvents.failed,
        `${readTool} completed through an OMP tool execution`,
      ),
      task_mutation: outcome(
        mutationEvents.started &&
          mutationEvents.ended &&
          !mutationEvents.failed &&
          plan.includes(TASK_NAME),
        'truenorth_record_task changed the disposable release plan',
      ),
      clean_shutdown: outcome(
        closed.code === 0 &&
          closed.signal === null &&
          serverLifecycle.exited &&
          !serverLifecycle.alive,
        'closing RPC stdin exited OMP zero and terminated its TrueNorth stdio child',
      ),
      workspace_isolation: outcome(
        beforeStatus === afterStatus && beforePlan === afterPlan,
        'repository status and release plan were unchanged during certification',
      ),
    };

    const evidence = {
      schema_version: 1,
      generated_on: new Date().toISOString().slice(0, 10),
      client: { name: 'Oh My Pi', version: ompVersion },
      server: {
        name: 'truenorth-mcp',
        version: options.expectedVersion,
        transport: 'stdio',
      },
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
        rpc_protocol_version: 2,
      },
      fixture: {
        profile: 'generic',
        omp_profile: PROFILE,
        packaged_wrapper: true,
        install_scripts: false,
        disposable_workspace: true,
        isolated_home: true,
      },
      rpc_evidence: {
        ready: {
          type: ready.type,
          protocolVersion: ready.protocolVersion,
          supportedProtocolVersions: ready.supportedProtocolVersions,
        },
        model: state.data?.model ?? null,
        tools: [readTool, mutationTool],
        mcp_list_observed: listResult.output.includes('truenorth'),
        state_resource_observed: resourceResult.output.includes('truenorth://state'),
        tool_events: summarizeToolEvents(promptFrames, [readTool, mutationTool]),
        terminal_agent_end: promptFrames.some(
          (frame) => frame.type === 'agent_end' && frame.isTerminal !== false,
        ),
        process_exit: closed,
        server_exit: {
          observed: serverLifecycle.exited,
          alive_after_omp_exit: serverLifecycle.alive,
        },
      },
      operations,
      certified: Object.values(operations).every((operation) => operation.status === 'pass'),
    };

    validateEvidence(evidence);
    fs.mkdirSync(options.output, { recursive: true });
    const outputPath = path.join(options.output, 'oh-my-pi.json');
    fs.writeFileSync(outputPath, `${JSON.stringify(evidence, null, 2)}\n`);
    console.log(`Oh My Pi certification wrote ${outputPath}`);
    console.log(`Oh My Pi ${ompVersion}: ${evidence.certified ? 'certified' : 'failed'}`);
    if (!evidence.certified) process.exitCode = 1;
  } finally {
    if (rpc) {
      rpc.terminate();
      await rpc.waitForExit(5_000).catch(() => undefined);
    }
    fixture.cleanup();
  }
}
function startRpc(options, fixture, config, sessions, env) {
  const args = [
    '--profile',
    PROFILE,
    '--tools',
    'mcp__truenorth_get_skill,mcp__truenorth_record_task',
    '--model',
    `llama.cpp/${options.modelId}`,
    '--thinking',
    'off',
    '--system-prompt',
    'Follow the user request exactly. Use only the provided MCP tools. Do not explain tool calls.',
    '--cwd',
    fixture.project,
    '--session-dir',
    sessions,
    '--config',
    config,
    '--mode',
    'rpc',
    '--no-session',
    '--auto-approve',
    '--no-rules',
    '--no-skills',
    '--no-extensions',
  ];
  const child = spawn(options.omp, args, {
    cwd: fixture.project,
    env,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  return createRpcClient(child);
}

function createRpcClient(child) {
  const frames = [];
  const waiters = new Set();
  const pending = new Map();
  let nextId = 1;
  let stderr = '';
  let settledExit = null;
  let resolveExit;
  const exitPromise = new Promise((resolve) => {
    resolveExit = resolve;
  });

  readline.createInterface({ input: child.stdout }).on('line', (line) => {
    if (!line.trim()) return;
    let frame;
    try {
      frame = JSON.parse(line);
    } catch (error) {
      rejectAll(new Error(`OMP emitted invalid RPC JSON: ${error.message}`));
      return;
    }
    frames.push(frame);
    if (frame.id && frame.type === 'response' && pending.has(frame.id)) {
      const request = pending.get(frame.id);
      pending.delete(frame.id);
      if (frame.success === false)
        request.reject(new Error(frame.error || `OMP request ${frame.id} failed`));
      else request.resolve(frame);
    }
    for (const waiter of [...waiters]) {
      if (waiter.predicate(frame, frames.length - 1)) {
        clearTimeout(waiter.timer);
        waiters.delete(waiter);
        waiter.resolve(frame);
      }
    }
  });
  child.stderr.on('data', (chunk) => {
    stderr += chunk.toString();
    if (stderr.length > 64 * 1024) stderr = stderr.slice(-64 * 1024);
  });
  child.on('error', (error) => rejectAll(error));
  child.on('exit', (code, signal) => {
    settledExit = { code, signal };
    resolveExit(settledExit);
    if (code !== 0 || signal) {
      rejectAll(new Error(`OMP exited ${code ?? signal}: ${stderr.trim()}`));
    }
  });

  function rejectAll(error) {
    for (const request of pending.values()) request.reject(error);
    pending.clear();
    for (const waiter of waiters) {
      clearTimeout(waiter.timer);
      waiter.reject(error);
    }
    waiters.clear();
  }

  return {
    frames,
    request(command) {
      const id = `cert-${nextId++}`;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        child.stdin.write(`${JSON.stringify({ id, ...command })}\n`);
      });
    },
    waitFor(predicate, timeoutMs) {
      const existing = frames.find((frame, index) => predicate(frame, index));
      if (existing) return Promise.resolve(existing);
      return new Promise((resolve, reject) => {
        const waiter = { predicate, resolve, reject };
        waiter.timer = setTimeout(() => {
          waiters.delete(waiter);
          const tail = JSON.stringify(
            frames.slice(-12).map((frame) => ({
              type: frame.type,
              toolName: eventToolName(frame),
              isTerminal: frame.isTerminal,
              success: frame.success,
              eventType: frame.assistantMessageEvent?.type,
            })),
          );
          reject(
            new Error(`timed out waiting for OMP RPC event after ${timeoutMs}ms; tail: ${tail}`),
          );
        }, timeoutMs);
        waiters.add(waiter);
      });
    },
    closeInput() {
      child.stdin.end();
    },
    terminate() {
      if (!settledExit) child.kill('SIGTERM');
    },
    async waitForExit(timeoutMs) {
      if (settledExit) return settledExit;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(
          () => reject(new Error(`OMP did not exit after ${timeoutMs}ms`)),
          timeoutMs,
        );
        exitPromise.then((result) => {
          clearTimeout(timer);
          resolve(result);
        });
      });
    },
  };
}
async function runAgentTurn(rpc, message, timeoutMs) {
  const start = rpc.frames.length;
  const response = await rpc.request({ type: 'prompt', message });
  if (response.data?.agentInvoked === false) {
    throw new Error('OMP handled a certification prompt without invoking the agent');
  }
  await rpc.waitFor(
    (frame, index) => index >= start && frame.type === 'agent_end' && frame.isTerminal !== false,
    timeoutMs,
  );
  return rpc.frames.slice(start);
}

async function runLocalCommand(rpc, message) {
  const start = rpc.frames.length;
  const response = await rpc.request({ type: 'prompt', message });
  return {
    response,
    output: rpc.frames
      .slice(start)
      .filter((frame) => frame.type === 'command_output')
      .map(commandOutputText)
      .join('\n'),
  };
}

function commandOutputText(frame) {
  if (typeof frame.output === 'string') return frame.output;
  if (typeof frame.content === 'string') return frame.content;
  if (typeof frame.message === 'string') return frame.message;
  return JSON.stringify(frame);
}

function findTool(tools, suffix) {
  return tools
    .map((tool) => tool?.name)
    .find((name) => typeof name === 'string' && name.endsWith(suffix));
}

function toolEvents(frames, toolName) {
  const relevant = frames.filter((frame) => eventToolName(frame) === toolName);
  return {
    started: relevant.some((frame) => frame.type === 'tool_execution_start'),
    ended: relevant.some((frame) => frame.type === 'tool_execution_end'),
    failed: relevant.some(
      (frame) =>
        frame.type === 'tool_execution_end' &&
        (frame.isError === true || frame.result?.isError === true),
    ),
  };
}

function eventToolName(frame) {
  return frame.toolName ?? frame.tool?.name ?? frame.name ?? null;
}

function summarizeToolEvents(frames, toolNames) {
  return frames
    .filter(
      (frame) =>
        ['tool_execution_start', 'tool_execution_end'].includes(frame.type) &&
        toolNames.includes(eventToolName(frame)),
    )
    .map((frame) => ({
      type: frame.type,
      toolName: eventToolName(frame),
      isError: frame.isError === true || frame.result?.isError === true,
    }));
}

function writeOmpConfiguration({
  config,
  fixture,
  home,
  monitor,
  serverStart,
  serverExit,
  options,
}) {
  const ompDirectory = path.join(fixture.project, '.omp');
  const profileDirectory = path.join(home, '.omp', 'profiles', PROFILE, 'agent');
  fs.mkdirSync(profileDirectory, { recursive: true });
  fs.writeFileSync(
    path.join(profileDirectory, 'models.yml'),
    `${JSON.stringify(
      {
        providers: {
          'llama.cpp': {
            baseUrl: options.baseUrl,
            api: 'openai-completions',
            auth: 'none',
            models: [
              {
                id: options.modelId,
                name: options.modelId,
                contextWindow: 32768,
                maxTokens: 512,
                supportsTools: true,
              },
            ],
          },
        },
      },
      null,
      2,
    )}\n`,
  );
  fs.mkdirSync(ompDirectory, { recursive: true });
  fs.writeFileSync(
    path.join(ompDirectory, 'mcp.json'),
    `${JSON.stringify(
      {
        mcpServers: {
          truenorth: {
            type: 'stdio',
            command: process.execPath,
            args: [monitor, fixture.args[0]],
            cwd: fixture.project,
            env: {
              TRUENORTH_ROOT: fixture.project,
              TRUENORTH_VERIFY_CMD: 'true',
              CERT_SERVER_START: serverStart,
              CERT_SERVER_EXIT: serverExit,
            },
          },
        },
      },
      null,
      2,
    )}\n`,
  );
  fs.writeFileSync(
    config,
    `${JSON.stringify(
      {
        mcp: { enableProjectConfig: true },
        tools: { xdev: false },
        ttsr: { enabled: false },
        features: { unexpectedStopDetection: 'off' },
        retry: {
          modelFallback: false,
          fallbackChains: {
            default: [],
            [`llama.cpp/${options.modelId}`]: [],
          },
        },
        modelRoles: { default: `llama.cpp/${options.modelId}` },
      },
      null,
      2,
    )}\n`,
  );
}

function writeMonitor(file) {
  fs.writeFileSync(
    file,
    `'use strict';\nconst { spawn } = require('node:child_process');\nconst fs = require('node:fs');\nconst child = spawn(process.execPath, [process.argv[2]], { env: process.env, stdio: 'inherit' });\nfs.writeFileSync(process.env.CERT_SERVER_START, JSON.stringify({ pid: child.pid }));\nlet stopping = false;\nfor (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(signal, () => { if (!stopping) { stopping = true; child.kill(signal); } });\nchild.on('exit', (code, signal) => { fs.writeFileSync(process.env.CERT_SERVER_EXIT, JSON.stringify({ code, signal })); process.exitCode = code ?? (signal ? 1 : 0); });\n`,
  );
}

function isolatedEnvironment(home, baseUrl) {
  const env = {
    HOME: home,
    PATH: process.env.PATH,
    TMPDIR: process.env.TMPDIR || os.tmpdir(),
    LANG: process.env.LANG || 'C.UTF-8',
    LLAMA_CPP_BASE_URL: baseUrl,
    NO_COLOR: '1',
  };
  if (process.env.LC_ALL) env.LC_ALL = process.env.LC_ALL;
  return env;
}

function preflightModel(baseUrl, modelId) {
  const catalog = spawnSync(
    'xh',
    ['--ignore-stdin', '--check-status', '--print=b', 'GET', `${baseUrl}/v1/models`],
    { encoding: 'utf8' },
  );
  if (catalog.error) throw new Error(`local model preflight failed: ${catalog.error.message}`);
  if (catalog.status !== 0) {
    throw new Error(`local model preflight failed: ${catalog.stderr.trim()}`);
  }
  const payload = JSON.parse(catalog.stdout);
  const models = [...(payload.data ?? []), ...(payload.models ?? [])];
  if (!models.some((model) => [model.id, model.name, model.model].includes(modelId))) {
    throw new Error(`local model endpoint does not expose ${modelId}`);
  }

  const toolProbe = spawnSync(
    'xh',
    [
      '--ignore-stdin',
      '--check-status',
      '--print=b',
      'POST',
      `${baseUrl}/v1/chat/completions`,
      `model=${modelId}`,
      'messages:=[{\"role\":\"user\",\"content\":\"Call certification_probe now.\"}]',
      'tools:=[{\"type\":\"function\",\"function\":{\"name\":\"certification_probe\",\"description\":\"Confirm native tool calls\",\"parameters\":{\"type\":\"object\",\"properties\":{}}}}]',
      'tool_choice=required',
      'max_tokens:=128',
      'temperature:=0',
      'stream:=false',
    ],
    { encoding: 'utf8', timeout: 120_000 },
  );
  if (toolProbe.error) {
    throw new Error(`local model tool-call preflight failed: ${toolProbe.error.message}`);
  }
  if (toolProbe.status !== 0) {
    throw new Error(`local model tool-call preflight failed: ${toolProbe.stderr.trim()}`);
  }
  const probe = JSON.parse(toolProbe.stdout);
  const toolCall = probe.choices?.[0]?.message?.tool_calls?.[0];
  if (
    probe.choices?.[0]?.finish_reason !== 'tool_calls' ||
    toolCall?.function?.name !== 'certification_probe'
  ) {
    throw new Error('local model endpoint did not return a native OpenAI tool call');
  }
}

function clientVersion(omp, env) {
  const output = execFileSync(omp, ['--version'], { env, encoding: 'utf8' }).trim();
  const match = output.match(/(?:omp\/)?(\d+\.\d+\.\d+)/);
  if (!match) throw new Error(`could not parse Oh My Pi version from ${JSON.stringify(output)}`);
  return match[1];
}

async function readServerLifecycle(startFile, exitFile, timeoutMs) {
  const deadline = Date.now() + Math.min(timeoutMs, 5_000);
  while ((!fs.existsSync(startFile) || !fs.existsSync(exitFile)) && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  if (!fs.existsSync(startFile)) return { exited: false, alive: false };
  const { pid } = JSON.parse(fs.readFileSync(startFile, 'utf8'));
  let alive = true;
  try {
    process.kill(pid, 0);
  } catch (error) {
    if (error.code === 'ESRCH') alive = false;
    else throw error;
  }
  return { exited: fs.existsSync(exitFile), alive };
}

function pass(detail) {
  return { status: 'pass', detail };
}

function outcome(passed, detail) {
  return { status: passed ? 'pass' : 'fail', detail };
}

function validateEvidence(evidence) {
  if (evidence.schema_version !== 1)
    throw new Error('certification evidence must use schema_version 1');
  for (const name of REQUIRED_OPERATIONS) {
    const operation = evidence.operations?.[name];
    if (!operation || !['pass', 'fail'].includes(operation.status)) {
      throw new Error(`certification evidence has no valid ${name} outcome`);
    }
  }
  const expectedCertified = REQUIRED_OPERATIONS.every(
    (name) => evidence.operations[name].status === 'pass',
  );
  if (evidence.certified !== expectedCertified) {
    throw new Error('certified must match the required operation outcomes');
  }
  if (evidence.model?.authentication !== 'none' || evidence.model?.hosted_fallback !== false) {
    throw new Error('OMP certification must use a keyless model with hosted fallback disabled');
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(evidence.generated_on)) {
    throw new Error('generated_on must be an ISO calendar date');
  }
  return evidence;
}

function gitStatus(repoRoot) {
  return execFileSync('git', ['status', '--porcelain=v1', '--untracked-files=all'], {
    cwd: repoRoot,
    encoding: 'utf8',
  });
}

function fileDigest(file) {
  if (!fs.existsSync(file)) return null;
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function parseArgs(argv) {
  const options = {
    omp: 'omp',
    baseUrl: 'http://127.0.0.1:18081',
    modelId: 'hermes-3-llama-3.1-8b',
    timeoutMs: 300_000,
  };
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!value) throw new Error(usage());
    if (flag === '--expected-version') options.expectedVersion = value;
    else if (flag === '--platform-package') options.platformPackage = path.resolve(value);
    else if (flag === '--output') options.output = path.resolve(value);
    else if (flag === '--omp') options.omp = value;
    else if (flag === '--base-url') options.baseUrl = value.replace(/\/$/, '');
    else if (flag === '--model-id') options.modelId = value;
    else if (flag === '--timeout-ms') options.timeoutMs = Number(value);
    else throw new Error(usage());
  }
  if (!options.expectedVersion || !options.platformPackage || !options.output) {
    throw new Error(usage());
  }
  if (!Number.isSafeInteger(options.timeoutMs) || options.timeoutMs <= 0) {
    throw new Error('--timeout-ms must be a positive integer');
  }
  return options;
}

function usage() {
  return 'usage: node scripts/certify-omp.js --expected-version <version> --platform-package <directory> --output <directory> [--omp <path>] [--base-url <url>] [--model-id <id>] [--timeout-ms <milliseconds>]';
}

module.exports = {
  REQUIRED_OPERATIONS,
  eventToolName,
  parseArgs,
  summarizeToolEvents,
  toolEvents,
  validateEvidence,
};

if (require.main === module) {
  main().catch((error) => {
    console.error(`Oh My Pi certification failed: ${error.message}`);
    process.exitCode = 1;
  });
}
