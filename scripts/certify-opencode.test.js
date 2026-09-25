'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  REQUIRED_OPERATIONS,
  completedTool,
  parseArgs,
  protocolEvidence,
  resourceUris,
  validateEvidence,
} = require('./certify-opencode.js');

function evidence(status = 'pass') {
  return {
    schema_version: 1,
    model: { authentication: 'none', hosted_fallback: false },
    operations: Object.fromEntries(
      REQUIRED_OPERATIONS.map((name) => [name, { status, detail: `${name} result` }]),
    ),
    certified: status === 'pass',
  };
}

test('complete keyless evidence is accepted as certified', () => {
  assert.equal(validateEvidence(evidence()).certified, true);
});

test('a missing required operation cannot produce a certificate', () => {
  const report = evidence();
  delete report.operations.resources_list;
  assert.throws(() => validateEvidence(report), /resources_list/);
});

test('not-exposed capability keeps the client uncertified', () => {
  const report = evidence();
  report.operations.resources_list.status = 'not_exposed';
  report.certified = false;
  assert.equal(validateEvidence(report).certified, false);
});

test('certified flag cannot disagree with operation outcomes', () => {
  const report = evidence();
  report.operations.task_mutation.status = 'fail';
  assert.throws(() => validateEvidence(report), /certified must match/);
});

test('hosted or authenticated model evidence is rejected', () => {
  const report = evidence();
  report.model.authentication = 'api-key';
  assert.throws(() => validateEvidence(report), /keyless local model/);
});

test('completed tool events require the named completed tool', () => {
  const events = [
    { type: 'tool_use', part: { tool: 'execute', state: { status: 'running' } } },
    { type: 'tool_use', part: { tool: 'execute', state: { status: 'completed' } } },
  ];
  assert.equal(completedTool(events, 'execute'), events[1]);
  assert.equal(completedTool(events, 'read'), undefined);
});

test('protocol evidence records MCP discovery responses', () => {
  assert.deepEqual(
    protocolEvidence([
      { direction: 'client_to_server', method: 'tools/list' },
      {
        direction: 'server_to_client',
        tools: ['get_skill', 'truenorth_record_task'],
        resources: [],
      },
      { direction: 'client_to_server', method: 'resources/list' },
      {
        direction: 'server_to_client',
        tools: [],
        resources: ['truenorth://state'],
      },
      {
        direction: 'client_to_server',
        method: 'tools/call',
        id: 3,
        tool_call: 'get_skill',
      },
      {
        direction: 'server_to_client',
        id: 3,
        tool_response: true,
        tool_error: false,
      },
    ]),
    {
      methods: ['resources/list', 'tools/call', 'tools/list'],
      tools: ['get_skill', 'truenorth_record_task'],
      resources: ['truenorth://state'],
      tool_calls: [{ tool: 'get_skill', status: 'completed' }],
    },
  );
});
test('resource URI extraction accepts the OpenCode catalog envelope', () => {
  assert.deepEqual(
    resourceUris({
      location: { directory: '/private/tmp/project' },
      data: {
        resources: [
          { server: 'truenorth', uri: 'truenorth://state' },
          { server: 'truenorth', uri: 'truenorth://cockpit' },
        ],
      },
    }),
    ['truenorth://state', 'truenorth://cockpit'],
  );
});

test('argument parser pins local defaults and accepts certification inputs', () => {
  assert.deepEqual(
    parseArgs([
      '--expected-version',
      '1.0.1',
      '--platform-package',
      'stage',
      '--output',
      'compatibility',
    ]),
    {
      expectedVersion: '1.0.1',
      platformPackage: 'stage',
      output: 'compatibility',
      opencode: 'opencode',
      baseUrl: 'http://127.0.0.1:18081',
      modelId: 'hermes-3-llama-3.1-8b',
      timeoutMs: 240000,
    },
  );
});
