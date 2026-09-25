'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const {
  REQUIRED_OPERATIONS,
  parseProtocolVersion,
  validateEvidence,
} = require('./certify-mcp-inspector.js');

function evidence(status = 'pass') {
  return {
    schema_version: 1,
    generated_on: '2026-09-25',
    operations: Object.fromEntries(
      REQUIRED_OPERATIONS.map((name) => [name, { status, detail: name }]),
    ),
    certified: status === 'pass',
  };
}

test('protocol version is extracted from Inspector initialization diagnostics', () => {
  const diagnostics =
    'peer_info=Some(InitializeRequestParams { protocol_version: ProtocolVersion("2025-11-25") })';
  assert.equal(parseProtocolVersion(diagnostics), '2025-11-25');
  assert.equal(parseProtocolVersion('no initialization record'), null);
});

test('complete passing evidence is accepted as certified', () => {
  assert.equal(validateEvidence(evidence()).certified, true);
});

test('a missing required operation cannot produce a certificate', () => {
  const report = evidence();
  delete report.operations.clean_shutdown;
  assert.throws(() => validateEvidence(report), /clean_shutdown/);
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
