#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { loadFixtures } = require('./jev-guard-pilot.js');
const audited = [
  {
    id: 'pilot-v2-01',
    target: 'src/check.rs',
    baseline: ['--lib'],
    baselineMissing: false,
    proposal: ['--lib'],
    proposalMissing: true,
  },
  {
    id: 'pilot-v2-02',
    target: 'src/check.rs',
    baseline: ['--lib'],
    baselineMissing: true,
    proposal: ['--lib'],
    proposalMissing: false,
  },
  {
    id: 'pilot-v2-03',
    target: 'src/fresh/decoder.rs',
    baseline: ['--lib'],
    baselineMissing: false,
    proposal: ['--bin', 'fresh_decoder'],
    proposalMissing: true,
  },
  {
    id: 'pilot-v2-04',
    target: 'src/decoder.rs',
    baseline: ['--lib'],
    baselineMissing: false,
    proposal: ['--bin', 'decoder'],
    proposalMissing: false,
  },
];

function put(root, relative, content) {
  const absolute = path.join(root, relative);
  fs.mkdirSync(path.dirname(absolute), { recursive: true });
  fs.writeFileSync(absolute, content, { flag: 'wx' });
}

function compile(root, selection, expectMissing, target) {
  const env = Object.fromEntries(
    ['PATH', 'RUSTUP_TOOLCHAIN']
      .filter((key) => process.env[key] !== undefined)
      .map((key) => [key, process.env[key]]),
  );
  env.HOME = path.dirname(root);
  env.RUSTUP_HOME = process.env.RUSTUP_HOME || path.join(os.homedir(), '.rustup');
  env.CARGO_HOME = path.join(path.dirname(root), 'cargo-home');
  env.CARGO_TARGET_DIR = path.join(path.dirname(root), 'target');
  env.CARGO_TERM_COLOR = 'never';
  const args = ['check', '--offline', '--message-format=json', ...selection];
  const result = spawnSync('cargo', args, {
    cwd: root,
    env,
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error || result.signal || (result.status !== 0 && !expectMissing)) {
    throw new Error(
      `cargo ${selection.join(' ')} failed unexpectedly: ${result.error?.message || result.stderr?.trim() || result.signal || result.status}`,
    );
  }
  const messages = result.stdout
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  const errors = messages.filter(
    (item) => item.reason === 'compiler-message' && item.message.level === 'error',
  );
  if (!expectMissing) {
    if (
      result.status !== 0 ||
      errors.length !== 0 ||
      !messages.some((item) => item.reason === 'build-finished' && item.success)
    ) {
      throw new Error(`cargo ${selection.join(' ')} did not compile cleanly`);
    }
    return 'compiled';
  }
  const missingImport = (item) =>
    item.message.code?.code === 'E0432' &&
    item.message.spans.some(
      (span) =>
        span.file_name === target &&
        span.text.some((line) => line.text.trim() === 'use absent_codec::Decoder;'),
    );
  if (result.status === 0 || errors.length === 0 || !errors.every(missingImport)) {
    throw new Error(`cargo ${selection.join(' ')} did not fail only on absent_codec E0432`);
  }
  return 'absent_codec E0432';
}

function auditCase(entry, spec) {
  if (
    entry.proposal.paths[0] !== spec.target ||
    entry.expected.findings.hallucinated_import !== spec.proposalMissing ||
    entry.expected.decision !== (entry.expected.findings.hallucinated_import ? 'annotate' : 'allow')
  ) {
    throw new Error(`${spec.id}: selected target or expected label drift`);
  }
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'jev-import-pilot-'));
  const root = path.join(temporary, 'project');
  try {
    fs.mkdirSync(root);
    fs.mkdirSync(path.join(temporary, 'cargo-home'));
    for (const file of entry.files) put(root, file.path, file.content);
    const baseline = compile(root, spec.baseline, spec.baselineMissing, spec.target);
    if (spec.id === 'pilot-v2-03' && fs.existsSync(path.dirname(path.join(root, spec.target)))) {
      throw new Error('case03 new binary parent must not exist on disk');
    }
    const proposed = path.join(root, spec.target);
    fs.mkdirSync(path.dirname(proposed), { recursive: true });
    fs.writeFileSync(proposed, entry.proposal.content);
    const proposal = compile(root, spec.proposal, spec.proposalMissing, spec.target);
    console.log(`${spec.id}: baseline ${baseline}; proposal ${proposal}`);
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

try {
  const cases = loadFixtures().cases;
  for (const [index, spec] of audited.entries()) {
    if (cases[index].case_id !== spec.id) throw new Error(`${spec.id}: case order changed`);
    auditCase(cases[index], spec);
  }
} catch (error) {
  console.error(`import preflight: ${error.message}`);
  process.exitCode = 1;
}
