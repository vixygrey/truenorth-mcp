'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const {
  PINNED_ZED_COMMIT,
  PINNED_ZED_VERSION,
  REQUIRED_OPERATIONS,
  TASK_NAME,
  cleanup,
  parseOptions,
  protocolEvidence,
  validateEvidence,
} = require('./certify-zed.js');

function evidence(resourceStatus = 'pass') {
  const exposed = resourceStatus !== 'not_exposed';
  const methods = ['initialize', 'tools/list'];
  if (exposed) methods.push('resources/list', 'resources/read');
  const operations = Object.fromEntries(
    REQUIRED_OPERATIONS.map((name) => [name, { status: 'pass', detail: name }]),
  );
  operations.resources_list.status = resourceStatus;
  operations.resource_read.status = resourceStatus;
  const certified = Object.values(operations).every((operation) => operation.status === 'pass');

  return {
    schema_version: 1,
    assessment: certified ? 'certified' : 'unsupported_for_full_certification',
    client: { name: 'Zed', version: PINNED_ZED_VERSION, commit: PINNED_ZED_COMMIT },
    model: { provider: 'Test Provider', id: 'test-model' },
    fixture: {
      disposable_workspace: true,
      isolated_user_data: true,
      workspace_local_configuration: true,
      user_global_configuration_unchanged: true,
      real_project_workspace_unchanged: true,
    },
    client_evidence: {
      protocol_methods: methods,
      required_tools: ['get_skill', 'truenorth_record_task'],
      protocol_resources: exposed ? ['truenorth://state'] : [],
      protocol_tool_calls: [
        { tool: 'get_skill', status: 'completed' },
        { tool: 'truenorth_record_task', status: 'completed' },
      ],
      fixture_release_plan: {
        task_count: 1,
        task_name: TASK_NAME,
        verify_command: 'true',
      },
    },
    operations,
    certified,
  };
}

test('committed Zed evidence is valid and records the unsupported resource surface', () => {
  const report = JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', 'compatibility', 'zed.json'), 'utf8'),
  );
  assert.equal(validateEvidence(report).certified, false);
  assert.equal(report.operations.resources_list.status, 'not_exposed');
  assert.equal(report.operations.resource_read.status, 'not_exposed');
});

test('complete passing evidence certifies Zed', () => {
  assert.equal(validateEvidence(evidence()).certified, true);
});

test('missing operation cannot produce valid evidence', () => {
  const report = evidence();
  delete report.operations.clean_shutdown;
  assert.throws(() => validateEvidence(report), /clean_shutdown/);
});

test('evidence requires successful model-mediated tool calls', () => {
  const report = evidence();
  report.client_evidence.protocol_tool_calls[1].status = 'error';
  assert.throws(() => validateEvidence(report), /completed truenorth_record_task/);
});

test('resource operations not exposed produce a valid unsupported assessment', () => {
  const report = evidence('not_exposed');
  assert.equal(validateEvidence(report).certified, false);
  assert.equal(report.assessment, 'unsupported_for_full_certification');
});

test('resource pass requires matching protocol evidence', () => {
  const report = evidence();
  report.client_evidence.protocol_methods = report.client_evidence.protocol_methods.filter(
    (method) => method !== 'resources/read',
  );
  assert.throws(() => validateEvidence(report), /resources\/read pass/);
});

test('evidence rejects operator-specific absolute paths', () => {
  const report = evidence();
  report.client_evidence.log = '/Users/operator/Library/Logs/Zed/Zed.log';
  assert.throws(() => validateEvidence(report), /operator-specific absolute path/);
});

test('protocol evidence summarizes discovery, resources, and request outcomes', () => {
  assert.deepEqual(
    protocolEvidence([
      {
        direction: 'client_to_server',
        method: 'initialize',
        id: 1,
        protocol_version: '2025-11-25',
        client_info: { name: 'Zed', version: PINNED_ZED_VERSION },
      },
      { direction: 'server_to_client', id: 1, response: true, response_error: false },
      { direction: 'client_to_server', method: 'tools/list', id: 2 },
      {
        direction: 'server_to_client',
        id: 2,
        tools: ['get_skill', 'truenorth_record_task'],
        response: true,
        response_error: false,
      },
      { direction: 'client_to_server', method: 'resources/list', id: 3 },
      {
        direction: 'server_to_client',
        id: 3,
        resources: ['truenorth://state'],
        response: true,
        response_error: false,
      },
      {
        direction: 'client_to_server',
        method: 'tools/call',
        id: 4,
        tool_call: 'get_skill',
      },
      { direction: 'server_to_client', id: 4, response: true, response_error: false },
    ]),
    {
      protocol_version: '2025-11-25',
      client_info: { name: 'Zed', version: PINNED_ZED_VERSION },
      methods: ['initialize', 'resources/list', 'tools/call', 'tools/list'],
      completed_methods: ['initialize', 'resources/list', 'tools/call', 'tools/list'],
      tools: ['get_skill', 'truenorth_record_task'],
      resources: ['truenorth://state'],
      tool_calls: [{ tool: 'get_skill', status: 'completed' }],
    },
  );
});

test('argument parser maps kebab-case flags and checks required inputs', () => {
  assert.deepEqual(
    parseOptions(
      [
        '--expected-version',
        '1.0.2',
        '--platform-package',
        '/tmp/platform',
        '--model-provider',
        'Zed',
      ],
      ['expectedVersion', 'platformPackage', 'modelProvider'],
    ),
    {
      expectedVersion: '1.0.2',
      platformPackage: '/tmp/platform',
      modelProvider: 'Zed',
    },
  );
  assert.throws(() => parseOptions([], ['session']), /--session/);
});

test('cleanup rejects a workspace outside the declared session', () => {
  const session = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-zed-cleanup-test-'));
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-zed-outside-test-'));
  try {
    fs.writeFileSync(
      path.join(session, 'certification-session.json'),
      `${JSON.stringify({
        kind: 'truenorth-zed-certification',
        session_root: session,
        workspace: outside,
        user_data: path.join(session, 'user-data'),
      })}\n`,
    );
    assert.throws(() => cleanup({ session }), /refusing to remove/);
  } finally {
    fs.rmSync(session, { recursive: true, force: true });
    fs.rmSync(outside, { recursive: true, force: true });
  }
});
