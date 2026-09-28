'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');

const { McpError, startSession } = require('./mcp-session.js');

const REPORT_SCHEMA_VERSION = 1;
const SUITE_SCHEMA_VERSION = 1;
const MAX_CAPTURE_BYTES = 64 * 1024;

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function validateManifest(manifest, suiteRoot) {
  assertObject(manifest, 'manifest');
  if (manifest.schema_version !== SUITE_SCHEMA_VERSION) {
    throw new Error(`manifest schema_version must be ${SUITE_SCHEMA_VERSION}`);
  }
  if (!Array.isArray(manifest.scenarios) || manifest.scenarios.length === 0) {
    throw new Error('manifest scenarios must be a non-empty array');
  }
  const ids = new Set();
  return manifest.scenarios.map((entry) => {
    assertObject(entry, 'manifest scenario');
    if (!validId(entry.id)) throw new Error(`invalid scenario id ${JSON.stringify(entry.id)}`);
    if (ids.has(entry.id)) throw new Error(`duplicate scenario id ${entry.id}`);
    ids.add(entry.id);
    const directory = path.join(suiteRoot, 'scenarios', entry.id);
    const scenarioPath = path.join(directory, 'scenario.json');
    const scenario = readJson(scenarioPath);
    validateScenario(scenario, entry.id, directory);
    return { ...entry, directory, scenario, scenarioPath };
  });
}

function validateScenario(scenario, id, directory) {
  assertObject(scenario, `scenario ${id}`);
  if (scenario.schema_version !== SUITE_SCHEMA_VERSION) {
    throw new Error(`scenario ${id} schema_version must be ${SUITE_SCHEMA_VERSION}`);
  }
  if (scenario.id !== id) throw new Error(`scenario ${id} repeats a different id`);
  if (!Array.isArray(scenario.modes) || scenario.modes.length === 0) {
    throw new Error(`scenario ${id} modes must be a non-empty array`);
  }
  for (const mode of scenario.modes) {
    if (!['deterministic', 'model'].includes(mode)) {
      throw new Error(`scenario ${id} has unknown mode ${mode}`);
    }
  }
  if (!Number.isInteger(scenario.timeout_ms) || scenario.timeout_ms < 100) {
    throw new Error(`scenario ${id} timeout_ms must be an integer of at least 100`);
  }
  for (const field of ['driver', 'grader', 'fixture']) {
    if (typeof scenario[field] !== 'string' || !scenario[field]) {
      throw new Error(`scenario ${id} ${field} must be a non-empty string`);
    }
    const target = path.resolve(directory, scenario[field]);
    if (!target.startsWith(`${directory}${path.sep}`) || !fs.existsSync(target)) {
      throw new Error(`scenario ${id} ${field} does not resolve inside its directory`);
    }
  }
  assertObject(scenario.expected_assertions, `scenario ${id} expected_assertions`);
  for (const mode of scenario.modes) {
    const assertions = scenario.expected_assertions[mode];
    if (!Array.isArray(assertions) || assertions.length === 0) {
      throw new Error(`scenario ${id} needs expected assertions for ${mode}`);
    }
    if (
      new Set(assertions).size !== assertions.length ||
      assertions.some((value) => !validId(value))
    ) {
      throw new Error(`scenario ${id} has invalid or duplicate ${mode} assertion ids`);
    }
  }
  const mutations = scenario.declared_mutations ?? [];
  if (!Array.isArray(mutations) || mutations.some((value) => !safeRelative(value))) {
    throw new Error(`scenario ${id} declared_mutations must contain safe relative paths`);
  }
}

function validId(value) {
  return typeof value === 'string' && /^[a-z0-9][a-z0-9-]*$/.test(value);
}

function safeRelative(value) {
  return (
    typeof value === 'string' &&
    value.length > 0 &&
    !path.isAbsolute(value) &&
    !value.split(/[\\/]/).includes('..')
  );
}

function assertObject(value, name) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${name} must be an object`);
  }
}

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function digestFile(file) {
  return sha256(fs.readFileSync(file));
}

function snapshotTree(root, options = {}) {
  const exclude = new Set(options.exclude ?? []);
  const files = {};
  walk(root, '', files, exclude);
  return files;
}

function walk(root, relative, files, exclude) {
  const directory = path.join(root, relative);
  for (const entry of fs
    .readdirSync(directory, { withFileTypes: true })
    .sort((a, b) => a.name.localeCompare(b.name))) {
    const child = relative ? `${relative}/${entry.name}` : entry.name;
    if ([...exclude].some((prefix) => child === prefix || child.startsWith(`${prefix}/`))) continue;
    const absolute = path.join(root, child);
    if (entry.isSymbolicLink()) {
      files[child] = `symlink:${fs.readlinkSync(absolute)}`;
    } else if (entry.isDirectory()) {
      walk(root, child, files, exclude);
    } else if (entry.isFile()) {
      files[child] = sha256(fs.readFileSync(absolute));
    }
  }
  return files;
}

function treeDigest(root) {
  return sha256(stableStringify(snapshotTree(root)));
}

function diffSnapshots(before, after) {
  const paths = new Set([...Object.keys(before), ...Object.keys(after)]);
  return [...paths].filter((entry) => before[entry] !== after[entry]).sort();
}

function stableStringify(value, spacing = 2) {
  return `${JSON.stringify(sortValue(value), null, spacing)}\n`;
}

function sortValue(value) {
  if (Array.isArray(value)) return value.map(sortValue);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, sortValue(value[key])]),
  );
}

function copyFixture(source, target) {
  fs.cpSync(source, target, { recursive: true, verbatimSymlinks: true });
}

async function runProcess(command, args, options = {}) {
  const timeoutMs = options.timeoutMs ?? 30_000;
  const child = spawn(command, args, {
    cwd: options.cwd,
    env: options.env ?? process.env,
    detached: process.platform !== 'win32',
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  let stdout = Buffer.alloc(0);
  let stderr = Buffer.alloc(0);
  child.stdout.on('data', (chunk) => {
    stdout = boundedAppend(stdout, chunk);
  });
  child.stderr.on('data', (chunk) => {
    stderr = boundedAppend(stderr, chunk);
  });
  if (options.input === undefined) child.stdin.end();
  else child.stdin.end(options.input);

  let timer;
  let timedOut = false;
  const result = await new Promise((resolve, reject) => {
    timer = setTimeout(() => {
      timedOut = true;
      killProcessTree(child.pid);
    }, timeoutMs);
    child.once('error', reject);
    child.once('exit', (code, signal) => resolve({ code, signal }));
  }).finally(() => clearTimeout(timer));
  if (timedOut) {
    throw new Error(`${options.label ?? command} timed out after ${timeoutMs} ms`);
  }
  return {
    ...result,
    stdout: stdout.toString('utf8'),
    stderr: stderr.toString('utf8'),
  };
}

function boundedAppend(current, chunk) {
  const combined = Buffer.concat([current, Buffer.from(chunk)]);
  return combined.length <= MAX_CAPTURE_BYTES
    ? combined
    : combined.subarray(combined.length - MAX_CAPTURE_BYTES);
}

function killProcessTree(pid) {
  try {
    process.kill(process.platform === 'win32' ? pid : -pid, 'SIGKILL');
  } catch (error) {
    if (error.code !== 'ESRCH') throw error;
  }
}

function normalizeValue(value, roots) {
  if (typeof value === 'string') {
    let normalized = value;
    for (const [root, replacement] of roots) {
      normalized = normalized.split(root).join(replacement);
    }
    return normalized;
  }
  if (Array.isArray(value)) return value.map((entry) => normalizeValue(entry, roots));
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [key, normalizeValue(entry, roots)]),
  );
}

async function executeScenarioDriver(input) {
  const scenario = input.scenario;
  const scenarioRoot = input.scenario_root;
  const workspace = path.join(scenarioRoot, scenario.workspace_subdir ?? 'workspace');
  const fixture = path.join(input.scenario_directory, scenario.fixture);
  copyFixture(fixture, workspace);
  const roots = [
    [scenarioRoot, '<scenario-root>'],
    [input.repo_root, '<source-root>'],
  ];
  const events = [];
  const sessions = new Map();
  const declaredMutations = new Set(scenario.declared_mutations ?? []);

  function event(type, detail) {
    events.push({ index: events.length + 1, type, ...normalizeValue(detail, roots) });
  }

  function assertContained(target) {
    const resolved = path.resolve(target);
    if (resolved !== scenarioRoot && !resolved.startsWith(`${scenarioRoot}${path.sep}`)) {
      throw new Error(`scenario path escapes disposable root: ${target}`);
    }
    return resolved;
  }

  const context = {
    mode: input.mode,
    repoRoot: input.repo_root,
    scenarioRoot,
    workspace,
    scenario,
    event,
    async cli(args, options = {}) {
      const cwd = assertContained(options.cwd ?? workspace);
      const result = await runProcess(input.binary, args, {
        cwd,
        env: { ...process.env, TRUENORTH_ROOT: cwd, ...options.env },
        timeoutMs: options.timeoutMs ?? scenario.timeout_ms,
        label: `runtime ${args.join(' ')}`,
      });
      event('cli', { args, cwd, result });
      if (result.code !== 0 && !options.expectFailure) {
        throw new Error(`runtime ${args.join(' ')} failed: ${result.stderr.trim()}`);
      }
      return result;
    },
    async command(command, args = [], options = {}) {
      const cwd = assertContained(options.cwd ?? workspace);
      const result = await runProcess(command, args, {
        cwd,
        env: { ...process.env, ...options.env },
        input: options.input,
        timeoutMs: options.timeoutMs ?? scenario.timeout_ms,
        label: options.label ?? command,
      });
      event('command', { command, args, cwd, label: options.label ?? command, result });
      if (result.code !== 0 && !options.expectFailure) {
        throw new Error(`${command} failed: ${result.stderr.trim()}`);
      }
      return result;
    },
    async git(args, options = {}) {
      return this.command('git', args, { ...options, label: `git ${args.join(' ')}` });
    },
    mutate(relative, contents, options = {}) {
      if (!declaredMutations.has(relative)) {
        throw new Error(`scenario did not declare mutation ${relative}`);
      }
      const root = assertContained(options.root ?? workspace);
      const target = path.resolve(root, relative);
      if (!target.startsWith(`${root}${path.sep}`))
        throw new Error(`mutation escapes root: ${relative}`);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, contents);
      event('fixture_mutation', {
        path: path.relative(scenarioRoot, target).replaceAll(path.sep, '/'),
      });
    },
    read(relative, options = {}) {
      const root = assertContained(options.root ?? workspace);
      const target = path.resolve(root, relative);
      if (!target.startsWith(`${root}${path.sep}`))
        throw new Error(`read escapes root: ${relative}`);
      return fs.readFileSync(target, 'utf8');
    },
    async startSession(name, options = {}) {
      if (sessions.has(name)) throw new Error(`session ${name} already exists`);
      const root = assertContained(options.root ?? workspace);
      const session = startSession(input.binary, [], root, {
        requestTimeoutMs: options.requestTimeoutMs ?? scenario.timeout_ms,
        exitTimeoutMs: 5_000,
        env: { RUST_LOG: 'error', ...options.env },
      });
      sessions.set(name, session);
      await session.request('initialize', {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'truenorth-methodology-eval', version: '1' },
      });
      session.notify('notifications/initialized', {});
      event('session_start', { name, root });
      return session;
    },
    async stopSession(name) {
      const session = sessions.get(name);
      if (!session) throw new Error(`session ${name} is not running`);
      await session.close();
      sessions.delete(name);
      event('session_stop', { name });
    },
    async callTool(sessionName, tool, args, options = {}) {
      const session = sessions.get(sessionName);
      if (!session) throw new Error(`session ${sessionName} is not running`);
      let result;
      try {
        result = await session.request('tools/call', { name: tool, arguments: args });
      } catch (error) {
        const failure = {
          name: error.name,
          message: error.message,
          code: error instanceof McpError ? error.code : null,
          data: error instanceof McpError ? error.data : null,
        };
        event('mcp_tool', { session: sessionName, tool, args, ok: false, error: failure });
        if (!options.expectError) throw error;
        return { ok: false, error: failure };
      }
      event('mcp_tool', { session: sessionName, tool, args, ok: true, result });
      if (options.expectError) throw new Error(`${tool} unexpectedly succeeded`);
      return { ok: true, result };
    },
    async runModel(prompt) {
      const command = process.env.TRUENORTH_EVAL_MODEL_COMMAND;
      if (!command) throw new Error('model command is unavailable');
      const result = await runProcess('/bin/sh', ['-c', command], {
        cwd: workspace,
        env: { ...process.env, TRUENORTH_ROOT: workspace },
        input: stableStringify({ prompt, workspace }),
        timeoutMs: scenario.timeout_ms,
        label: 'model adapter',
      });
      event('model', { result });
      if (result.code !== 0) throw new Error(`model adapter failed: ${result.stderr.trim()}`);
      return result;
    },
  };

  let driverError = null;
  try {
    const driver = require(path.join(input.scenario_directory, scenario.driver));
    await driver.run(context);
  } catch (error) {
    driverError = { name: error.name, message: normalizeValue(error.message, roots) };
  } finally {
    for (const [name, session] of [...sessions]) {
      try {
        await session.terminate();
        event('session_terminate', { name });
      } catch (error) {
        event('session_terminate', { name, error: error.message });
      }
    }
  }

  return {
    schema_version: REPORT_SCHEMA_VERSION,
    scenario_id: scenario.id,
    mode: input.mode,
    workspace,
    scenario_root: scenarioRoot,
    events,
    driver_error: driverError,
  };
}

function definitionDigests(entry) {
  const scenario = entry.scenario;
  return {
    scenario: digestFile(entry.scenarioPath),
    fixture: treeDigest(path.join(entry.directory, scenario.fixture)),
    driver: digestFile(path.join(entry.directory, scenario.driver)),
    grader: digestFile(path.join(entry.directory, scenario.grader)),
  };
}

function baselineProjection(report) {
  return {
    schema_version: SUITE_SCHEMA_VERSION,
    expected_scenario_count: report.scenarios.length,
    scenarios: report.scenarios.map((scenario) => ({
      id: scenario.id,
      definition_digests: scenario.definition_digests,
      assertions: scenario.assertions.map(({ id, passed }) => ({ id, passed })),
    })),
  };
}

function modelAvailable() {
  return Boolean(
    process.env.TRUENORTH_EVAL_MODEL_COMMAND &&
    process.env.TRUENORTH_EVAL_MODEL_PROVIDER &&
    process.env.TRUENORTH_EVAL_MODEL_ID,
  );
}

function temporaryRoot(prefix = 'truenorth-methodology-') {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

module.exports = {
  REPORT_SCHEMA_VERSION,
  SUITE_SCHEMA_VERSION,
  baselineProjection,
  copyFixture,
  definitionDigests,
  diffSnapshots,
  executeScenarioDriver,
  modelAvailable,
  normalizeValue,
  readJson,
  runProcess,
  sha256,
  snapshotTree,
  stableStringify,
  temporaryRoot,
  validateManifest,
};
