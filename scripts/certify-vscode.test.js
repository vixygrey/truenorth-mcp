'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const {
  PINNED_COPILOT_VERSION,
  PINNED_VSCODE_VERSION,
  REQUIRED_OPERATIONS,
  parseOptions,
  protocolEvidence,
  validateEvidence,
} = require('./certify-vscode.js');

function evidence(status = 'pass') {
  return {
    schema_version: 1,
    client: {
      name: 'Visual Studio Code',
      version: PINNED_VSCODE_VERSION,
      extension: { name: 'GitHub Copilot Chat', version: PINNED_COPILOT_VERSION },
    },
    fixture: {
      disposable_workspace: true,
      disposable_vscode_profile: true,
      isolated_home: false,
      workspace_local_configuration: true,
      normal_vscode_profile_unchanged: true,
      real_project_workspace_unchanged: true,
      host_authentication_provider_reused: true,
    },
    client_evidence: {
      protocol_methods: ['initialize', 'resources/list', 'tools/list'],
      required_tools: ['get_skill', 'truenorth_record_task'],
      protocol_resources: ['truenorth://state'],
      protocol_tool_calls: [
        { tool: 'get_skill', status: 'completed' },
        { tool: 'truenorth_record_task', status: 'completed' },
      ],
    },
    operations: Object.fromEntries(
      REQUIRED_OPERATIONS.map((name) => [name, { status, detail: name }]),
    ),
    certified: status === 'pass',
  };
}

test('committed VS Code certificate is valid', () => {
  const certificate = JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', 'compatibility', 'vscode-copilot.json'), 'utf8'),
  );
  assert.equal(validateEvidence(certificate).certified, true);
});

test('missing operation cannot produce a certificate', () => {
  const report = evidence();
  delete report.operations.clean_shutdown;
  assert.throws(() => validateEvidence(report), /clean_shutdown/);
});

test('certificate requires successful model-mediated tool calls', () => {
  const report = evidence();
  report.client_evidence.protocol_tool_calls[1].status = 'error';
  assert.throws(() => validateEvidence(report), /completed truenorth_record_task/);
});

test('certificate rejects operator-specific absolute paths', () => {
  const report = evidence();
  report.client_evidence.log = '/Users/operator/Library/Application Support/Code/log.txt';
  assert.throws(() => validateEvidence(report), /operator-specific absolute path/);
});

test('protocol evidence summarizes discovery and tool results', () => {
  assert.deepEqual(
    protocolEvidence([
      {
        direction: 'client_to_server',
        method: 'initialize',
        id: 1,
        protocol_version: '2025-11-25',
        client_info: { name: 'Visual Studio Code', version: PINNED_VSCODE_VERSION },
      },
      { direction: 'server_to_client', id: 1, tool_response: true, tool_error: false },
      { direction: 'client_to_server', method: 'tools/list', id: 2 },
      {
        direction: 'server_to_client',
        id: 2,
        tools: ['get_skill', 'truenorth_record_task'],
        tool_response: true,
        tool_error: false,
      },
      {
        direction: 'client_to_server',
        method: 'tools/call',
        id: 3,
        tool_call: 'get_skill',
      },
      { direction: 'server_to_client', id: 3, tool_response: true, tool_error: false },
    ]),
    {
      protocol_version: '2025-11-25',
      client_info: { name: 'Visual Studio Code', version: PINNED_VSCODE_VERSION },
      methods: ['initialize', 'tools/call', 'tools/list'],
      tools: ['get_skill', 'truenorth_record_task'],
      resources: [],
      tool_calls: [{ tool: 'get_skill', status: 'completed' }],
    },
  );
});

test('argument parser maps kebab-case flags and checks required inputs', () => {
  assert.deepEqual(
    parseOptions(
      [
        '--expected-version',
        '1.0.1',
        '--platform-package',
        '/tmp/platform',
        '--vscode-app',
        '/tmp/Visual Studio Code.app',
      ],
      ['expectedVersion', 'platformPackage', 'vscodeApp'],
    ),
    {
      expectedVersion: '1.0.1',
      platformPackage: '/tmp/platform',
      vscodeApp: '/tmp/Visual Studio Code.app',
    },
  );
  assert.throws(() => parseOptions([], ['session']), /--session/);
});
