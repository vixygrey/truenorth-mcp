'use strict';

const assert = require('node:assert');
const { execFileSync } = require('node:child_process');
const path = require('node:path');
const { test } = require('node:test');

const packageRoot = path.resolve(__dirname, '..');

test('the packed wrapper publishes its README', () => {
  const output = execFileSync('npm', ['pack', '--dry-run', '--json'], {
    cwd: packageRoot,
    encoding: 'utf8',
  });
  const [pack] = JSON.parse(output);

  assert.ok(pack.files.some((file) => file.path === 'README.md'));
});
