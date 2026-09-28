'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { afterEach, test } = require('node:test');
const { discover, run } = require('./verify-scripted-skills.js');

const roots = [];
afterEach(() => {
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function fixture(skills) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-scripted-skills-'));
  roots.push(root);
  for (const [name, metadata] of Object.entries(skills)) {
    const dir = path.join(root, 'skills', name);
    fs.mkdirSync(dir, { recursive: true });
    const verify = metadata.verify === undefined ? '' : `verify: ${metadata.verify}\n`;
    fs.writeFileSync(
      path.join(dir, 'SKILL.md'),
      `---\nname: ${name}\ndescription: fixture\nkind: ${metadata.kind}\n${verify}---\n`,
    );
  }
  return root;
}

function sink() {
  let value = '';
  return {
    stream: {
      write(chunk) {
        value += chunk;
      },
    },
    value: () => value,
  };
}

test('discovers scripted skills in stable name order', () => {
  const root = fixture({
    zebra: { kind: 'scripted', verify: 'true' },
    prose: { kind: 'prose' },
    alpha: { kind: 'scripted', verify: 'true' },
  });
  assert.deepStrictEqual(
    discover(root).map((skill) => skill.name),
    ['alpha', 'zebra'],
  );
});

test('fails when a scripted skill has no verify command', () => {
  const root = fixture({ broken: { kind: 'scripted' } });
  assert.strictEqual(run(root, sink().stream, sink().stream), 1);
});

test('propagates command failure', () => {
  const root = fixture({ broken: { kind: 'scripted', verify: 'exit 7' } });
  assert.strictEqual(run(root, sink().stream, sink().stream), 1);
});

test('treats unavailable as non-passing', () => {
  const root = fixture({
    browser: {
      kind: 'scripted',
      verify: "printf 'TRUENORTH_VERIFY_UNAVAILABLE: Chrome missing\\n'; exit 1",
    },
  });
  const output = sink();
  const errors = sink();
  assert.strictEqual(run(root, output.stream, errors.stream), 1);
  assert.match(errors.value(), /UNAVAILABLE browser \(Chrome missing\)/);
});

test('passes only when every scripted command exits zero', () => {
  const root = fixture({
    alpha: { kind: 'scripted', verify: 'true' },
    beta: { kind: 'scripted', verify: 'true' },
  });
  assert.strictEqual(run(root, sink().stream, sink().stream), 0);
});
