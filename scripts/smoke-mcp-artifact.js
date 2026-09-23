'use strict';

const { execFileSync, spawn } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const readline = require('node:readline');

const REQUEST_TIMEOUT_MS = 10_000;
const EXIT_TIMEOUT_MS = 5_000;

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.platformPackage) {
    await smokePackedWrapper(options.expectedVersion, options.platformPackage);
  } else {
    await smokeCommand(options.expectedVersion, options.command, options.args);
  }
}

async function smokePackedWrapper(expectedVersion, platformPackage) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-packed-mcp-smoke-'));
  try {
    const packs = path.join(work, 'packs');
    const install = path.join(work, 'install');
    fs.mkdirSync(packs);
    fs.mkdirSync(install);
    fs.writeFileSync(path.join(install, 'package.json'), '{"private":true}\n');

    const platformTarball = pack(platformPackage, packs);
    const wrapperTarball = pack('npm', packs);
    execFileSync(
      'npm',
      [
        'install',
        '--ignore-scripts',
        '--no-audit',
        '--no-fund',
        '--offline',
        platformTarball,
        wrapperTarball,
      ],
      { cwd: install, stdio: 'inherit' },
    );

    const project = path.join(work, 'project');
    fs.mkdirSync(project);
    const wrapper = path.join(install, 'node_modules', 'truenorth-mcp', 'bin', 'truenorth.js');
    bootstrapWorkspace(process.execPath, [wrapper, 'init', '--profile', 'generic'], project);
    checkConfiguration(process.execPath, [wrapper, '--check-config'], project);
    await smokeCommand(expectedVersion, process.execPath, [wrapper], project, true);
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

function pack(packageDirectory, destination) {
  const output = execFileSync(
    'npm',
    ['pack', '--json', '--pack-destination', destination, path.resolve(packageDirectory)],
    { encoding: 'utf8' },
  );
  const [{ filename }] = JSON.parse(output);
  return path.join(destination, filename);
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
        name: 'truenorth_get_skill',
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

function bootstrapWorkspace(command, args, root) {
  execFileSync(command, args, {
    cwd: root,
    env: { ...process.env, TRUENORTH_ROOT: root, TRUENORTH_VERIFY_CMD: 'true' },
    stdio: 'inherit',
  });
}

function checkConfiguration(command, args, root) {
  execFileSync(command, args, {
    cwd: root,
    env: { ...process.env, TRUENORTH_ROOT: root, TRUENORTH_VERIFY_CMD: 'true' },
    stdio: 'inherit',
  });
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

function startSession(command, args, root) {
  const executable = command.includes(path.sep) ? path.resolve(command) : command;
  const child = spawn(executable, args, {
    cwd: root,
    env: { ...process.env, TRUENORTH_ROOT: root },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const pending = new Map();
  const stderr = [];
  let nextId = 1;
  let processError;
  let exited = false;

  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => stderr.push(chunk));
  child.on('error', (error) => {
    processError = error;
    rejectPending(pending, formatFailure(`could not start ${command}: ${error.message}`, stderr));
  });
  child.on('exit', (code, signal) => {
    exited = true;
    if (code !== 0) {
      rejectPending(
        pending,
        formatFailure(
          `${command} exited with code ${code ?? 'none'}${signal ? ` (${signal})` : ''}`,
          stderr,
        ),
      );
    }
  });

  const lines = readline.createInterface({ input: child.stdout });
  lines.on('line', (line) => {
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      rejectPending(pending, formatFailure(`received invalid MCP JSON: ${line}`, stderr));
      return;
    }

    if (message.id === undefined || !pending.has(message.id)) {
      return;
    }

    const request = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) {
      request.reject(
        formatFailure(`MCP ${request.method} failed: ${message.error.message}`, stderr),
      );
      return;
    }
    request.resolve(message.result);
  });

  return {
    request(method, params) {
      if (processError || exited) {
        return Promise.reject(
          formatFailure(`cannot send ${method}: process is not running`, stderr),
        );
      }

      const id = nextId++;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          pending.delete(id);
          reject(formatFailure(`timed out waiting for MCP ${method}`, stderr));
        }, REQUEST_TIMEOUT_MS);
        pending.set(id, {
          method,
          resolve: (result) => {
            clearTimeout(timer);
            resolve(result);
          },
          reject: (error) => {
            clearTimeout(timer);
            reject(error);
          },
        });
        child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
      });
    },
    notify(method, params) {
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
    },
    async close() {
      if (exited) {
        return;
      }
      child.stdin.end();
      await waitForExit(child, stderr);
    },
    async terminate() {
      if (!exited) {
        child.kill();
      }
    },
  };
}

function waitForExit(child, stderr) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(formatFailure('MCP process did not exit after stdin closed', stderr));
    }, EXIT_TIMEOUT_MS);
    child.once('exit', (code, signal) => {
      clearTimeout(timer);
      if (code === 0) {
        resolve();
        return;
      }
      reject(
        formatFailure(
          `MCP process exited with code ${code ?? 'none'}${signal ? ` (${signal})` : ''}`,
          stderr,
        ),
      );
    });
  });
}

function rejectPending(pending, error) {
  for (const request of pending.values()) {
    request.reject(error);
  }
  pending.clear();
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

function formatFailure(message, stderr) {
  const output = stderr.join('').trim();
  return new Error(output ? `${message}\nstderr:\n${output}` : message);
}

main().catch((error) => {
  console.error(`MCP artifact smoke failed: ${error.message}`);
  process.exitCode = 1;
});
