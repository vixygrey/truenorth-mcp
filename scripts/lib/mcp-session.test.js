'use strict';

const assert = require('node:assert/strict');
const { test } = require('node:test');

const { McpError, startSession } = require('./mcp-session.js');

const ROOT = process.cwd();

function nodeSession(source, options = {}) {
  return startSession(process.execPath, ['-e', source], ROOT, options);
}

test('structured MCP errors preserve code and data', async () => {
  const session = nodeSession(`
    const readline = require('node:readline');
    readline.createInterface({ input: process.stdin }).on('line', (line) => {
      const request = JSON.parse(line);
      process.stdout.write(JSON.stringify({
        jsonrpc: '2.0',
        id: request.id,
        error: { code: -32600, message: 'contended', data: { type: 'writer_lease_conflict' } }
      }) + '\\n');
    });
  `);
  try {
    await assert.rejects(session.request('tools/call', {}), (error) => {
      assert.ok(error instanceof McpError);
      assert.equal(error.method, 'tools/call');
      assert.equal(error.code, -32600);
      assert.deepEqual(error.data, { type: 'writer_lease_conflict' });
      return true;
    });
  } finally {
    await session.terminate();
  }
});

test('request timeout terminates the child before rejecting', async () => {
  const session = nodeSession('process.stdin.resume();', {
    requestTimeoutMs: 25,
    exitTimeoutMs: 1_000,
  });
  await assert.rejects(session.request('tools/list', {}), /timed out waiting for MCP tools\/list/);
  assert.throws(() => process.kill(session.pid, 0), /ESRCH/);
});

test('unexpected clean exit rejects pending requests', async () => {
  const session = nodeSession(`
    process.stdin.once('data', () => process.exit(0));
  `);
  await assert.rejects(session.request('tools/list', {}), /exited with code 0/);
});

test('close accepts a clean exit and terminate waits for a signaled exit', async () => {
  const clean = nodeSession(
    'process.stdin.resume(); process.stdin.on("end", () => process.exit(0));',
  );
  await clean.close();

  const terminated = nodeSession('process.stdin.resume();', { exitTimeoutMs: 1_000 });
  const result = await terminated.terminate();
  assert.equal(result.signal, 'SIGTERM');
});
