'use strict';

const { spawn } = require('node:child_process');
const path = require('node:path');
const readline = require('node:readline');

const DEFAULT_REQUEST_TIMEOUT_MS = 10_000;
const DEFAULT_EXIT_TIMEOUT_MS = 5_000;

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
      request.reject(
        formatFailure(`MCP ${request.method} failed: ${message.error.message}`, stderr),
      );
      return;
    }
    request.resolve(message.result);
  });

  return {
    pid: child.pid,
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
      if (exited) {
        return;
      }
      child.stdin.end();
      await waitForExit(child, stderr, exitTimeoutMs);
    },
    async terminate() {
      if (!exited) {
        child.kill();
      }
    },
  };
}

function waitForExit(child, stderr, timeoutMs) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(formatFailure('MCP process did not exit after stdin closed', stderr));
    }, timeoutMs);
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

function formatFailure(message, stderr) {
  const output = stderr.join('').trim();
  return new Error(output ? `${message}\nstderr:\n${output}` : message);
}

module.exports = { startSession };
