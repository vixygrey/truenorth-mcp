'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const {
  REQUIRED_OPERATIONS,
  eventToolName,
  parseArgs,
  summarizeToolEvents,
  toolEvents,
  validateEvidence,
} = require('./certify-omp.js');

function evidence(status = 'pass') {
  return {
    schema_version: 1,
    generated_on: '2026-09-25',
    model: {
      provider: 'llama.cpp',
      id: 'qwen2.5-coder:14b',
      authentication: 'none',
      hosted_fallback: false,
    },
    operations: Object.fromEntries(
      REQUIRED_OPERATIONS.map((name) => [name, { status, detail: name }]),
    ),
    certified: status === 'pass',
  };
}

test('complete keyless evidence is accepted as certified', () => {
  assert.equal(validateEvidence(evidence()).certified, true);
});

test('a missing required operation cannot produce a certificate', () => {
  const report = evidence();
  delete report.operations.clean_shutdown;
  assert.throws(() => validateEvidence(report), /clean_shutdown/);
});

test('hosted fallback invalidates otherwise passing evidence', () => {
  const report = evidence();
  report.model.hosted_fallback = true;
  assert.throws(() => validateEvidence(report), /keyless model/);
});

test('certified flag cannot disagree with operation outcomes', () => {
  const report = evidence();
  report.operations.task_mutation.status = 'fail';
  assert.throws(() => validateEvidence(report), /certified must match/);
});

test('OMP tool lifecycle evidence identifies successful and failed calls', () => {
  const frames = [
    { type: 'tool_execution_start', toolName: 'mcp__truenorth_get_skill' },
    {
      type: 'tool_execution_end',
      toolName: 'mcp__truenorth_get_skill',
      result: { isError: false },
    },
    { type: 'tool_execution_start', tool: { name: 'mcp__truenorth_record_task' } },
    {
      type: 'tool_execution_end',
      tool: { name: 'mcp__truenorth_record_task' },
      isError: true,
    },
  ];

  assert.deepEqual(toolEvents(frames, 'mcp__truenorth_get_skill'), {
    started: true,
    ended: true,
    failed: false,
  });
  assert.deepEqual(toolEvents(frames, 'mcp__truenorth_record_task'), {
    started: true,
    ended: true,
    failed: true,
  });
  assert.equal(eventToolName(frames[2]), 'mcp__truenorth_record_task');
  assert.deepEqual(
    summarizeToolEvents(frames, ['mcp__truenorth_get_skill', 'mcp__truenorth_record_task']),
    [
      {
        type: 'tool_execution_start',
        toolName: 'mcp__truenorth_get_skill',
        isError: false,
      },
      {
        type: 'tool_execution_end',
        toolName: 'mcp__truenorth_get_skill',
        isError: false,
      },
      {
        type: 'tool_execution_start',
        toolName: 'mcp__truenorth_record_task',
        isError: false,
      },
      {
        type: 'tool_execution_end',
        toolName: 'mcp__truenorth_record_task',
        isError: true,
      },
    ],
  );
});

test('argument parser pins local defaults and accepts certification inputs', () => {
  assert.deepEqual(
    parseArgs([
      '--expected-version',
      '1.0.1',
      '--platform-package',
      'npm/packages/truenorth-mcp-darwin-arm64',
      '--output',
      'compatibility',
    ]),
    {
      expectedVersion: '1.0.1',
      platformPackage: require('node:path').resolve('npm/packages/truenorth-mcp-darwin-arm64'),
      output: require('node:path').resolve('compatibility'),
      omp: 'omp',
      baseUrl: 'http://127.0.0.1:18081',
      modelId: 'hermes-3-llama-3.1-8b',
      timeoutMs: 300_000,
    },
  );
});
