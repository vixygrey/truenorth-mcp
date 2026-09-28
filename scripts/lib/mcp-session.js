'use strict';

const { spawn } = require('node:child_process');
const path = require('node:path');
const readline = require('node:readline');

const DEFAULT_REQUEST_TIMEOUT_MS = 10_000;
const DEFAULT_EXIT_TIMEOUT_MS = 5_000;

class McpError extends Error {
  constructor(method, response, stderr) {
    super(withStderr(`MCP ${method} failed: ${response.message}`, stderr));
    this.name = 'McpError';
    this.method = method;
    this.code = response.code;
    this.data = response.data;
  }
}

function startSession(command, args, root, options = {}) {
  const executable = command.includes(path.sep) ? path.resolve(command) : command;
  const requestTimeoutMs = options.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS;
  const exitTimeoutMs = options.exitTimeoutMs ?? DEFAULT_EXIT_TIMEOUT_MS;
  const child = spawn(executable, args, {
    cwd: root,
    env: { ...process.env, TRUENORTH_ROOT: root, ...options.env },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const pending = new Map();
  const notificationListeners = new Set();
  const stderr = [];
  let nextId = 1;
  let processError;
  let exitResult;

  const exited = new Promise((resolve) => {
    child.once('exit', (code, signal) => {
      exitResult = { code, signal };
      rejectPending(
        pending,
        formatFailure(
          `${command} exited with code ${code ?? 'none'}${signal ? ` (${signal})` : ''}`,
          stderr,
        ),
      );
      resolve(exitResult);
    });
  });

  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => stderr.push(chunk));
  child.on('error', (error) => {
    processError = error;
    rejectPending(pending, formatFailure(`could not start ${command}: ${error.message}`, stderr));
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

    if (message.id === undefined) {
      for (const listener of notificationListeners) {
        listener(message);
      }
      return;
    }
    if (!pending.has(message.id)) {
      return;
    }

    const request = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) {
      request.reject(new McpError(request.method, message.error, stderr));
      return;
    }
    request.resolve(message.result);
  });

  async function terminateProcess() {
    if (exitResult) {
      return exitResult;
    }
    child.kill('SIGTERM');
    try {
      return await withTimeout(exited, exitTimeoutMs, 'MCP process did not exit after SIGTERM');
    } catch {
      if (!exitResult) {
        child.kill('SIGKILL');
      }
      return withTimeout(exited, exitTimeoutMs, 'MCP process did not exit after SIGKILL');
    }
  }

  return {
    pid: child.pid,
    request(method, params) {
      if (processError || exitResult) {
        return Promise.reject(
          formatFailure(`cannot send ${method}: process is not running`, stderr),
        );
      }

      const id = nextId++;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          pending.delete(id);
          const failure = formatFailure(`timed out waiting for MCP ${method}`, stderr);
          terminateProcess().then(
            () => reject(failure),
            (error) => reject(error),
          );
        }, requestTimeoutMs);
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
      if (processError || exitResult) {
        throw formatFailure(`cannot send ${method}: process is not running`, stderr);
      }
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
    },
    waitForNotification(predicate, timeoutMs = requestTimeoutMs) {
      return new Promise((resolve, reject) => {
        const listener = (message) => {
          if (!predicate(message)) {
            return;
          }
          clearTimeout(timer);
          notificationListeners.delete(listener);
          resolve(message);
        };
        const timer = setTimeout(() => {
          notificationListeners.delete(listener);
          reject(formatFailure('timed out waiting for MCP notification', stderr));
        }, timeoutMs);
        notificationListeners.add(listener);
      });
    },
    async close() {
      if (!exitResult) {
        child.stdin.end();
      }
      const result =
        exitResult ??
        (await withTimeout(exited, exitTimeoutMs, 'MCP process did not exit after stdin closed'));
      if (result.code !== 0) {
        throw formatFailure(
          `MCP process exited with code ${result.code ?? 'none'}${
            result.signal ? ` (${result.signal})` : ''
          }`,
          stderr,
        );
      }
    },
    terminate: terminateProcess,
  };
}

function withTimeout(promise, timeoutMs, message) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(message)), timeoutMs);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

function rejectPending(pending, error) {
  for (const request of pending.values()) {
    request.reject(error);
  }
  pending.clear();
}

function withStderr(message, stderr) {
  const output = stderr.join('').trim();
  return output ? `${message}\nstderr:\n${output}` : message;
}

function formatFailure(message, stderr) {
  return new Error(withStderr(message, stderr));
}

module.exports = { McpError, startSession };
