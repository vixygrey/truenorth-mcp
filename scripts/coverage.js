'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync, spawnSync } = require('node:child_process');
const { METRIC, hash, sourceSnapshot, evaluate } = require('./coverage-report.js');

function options(args) {
  const action = args.shift();
  if (!['run', 'check'].includes(action))
    throw new Error('usage: node scripts/coverage.js run|check [--root DIR] [--report DIR]');
  let root = path.resolve(__dirname, '..');
  let directory;
  const seen = new Set();
  while (args.length) {
    const flag = args.shift();
    if (
      !['--root', '--report'].includes(flag) ||
      seen.has(flag) ||
      !args[0] ||
      args[0].startsWith('--')
    )
      throw new Error(`invalid coverage option ${flag}`);
    seen.add(flag);
    const value = path.resolve(args.shift());
    if (flag === '--root') root = value;
    else directory = value;
  }
  return { action, root, directory: directory || path.join(root, 'runtime/target/coverage') };
}

function command(root, binary, args) {
  const result = spawnSync(binary, args, { cwd: root, stdio: 'inherit' });
  if (result.error) throw new Error(`${binary}: ${result.error.message}`);
  if (result.status !== 0)
    throw new Error(
      `${binary} ${args.join(' ')}: measurement failed with ${result.signal || `exit ${result.status}`}; no coverage acceptance report was issued`,
    );
}

function measure(root, directory) {
  if (Number(process.versions.node.split('.')[0]) < 22)
    throw new Error('coverage requires Node 22 or newer for the built-in V8 LCOV reporter');
  const llvm = execFileSync('cargo', ['llvm-cov', '--version'], {
    cwd: root,
    encoding: 'utf8',
  }).trim();
  const rust = execFileSync('rustc', ['--version'], { cwd: root, encoding: 'utf8' }).trim();
  if (llvm !== 'cargo-llvm-cov 0.9.1')
    throw new Error(`coverage tool: found ${llvm}; install the approved cargo-llvm-cov 0.9.1`);
  if (!rust.startsWith('rustc 1.88.0 '))
    throw new Error(
      `coverage compiler: found ${rust}; use the pinned Rust 1.88.0 toolchain with llvm-tools-preview`,
    );
  fs.mkdirSync(directory, { recursive: true });
  fs.rmSync(path.join(directory, 'report.json'), { force: true });
  const before = sourceSnapshot(root);
  command(root, 'cargo', [
    'llvm-cov',
    'clean',
    '--workspace',
    '--manifest-path',
    'runtime/Cargo.toml',
  ]);
  const base = ['llvm-cov', '--locked', '--manifest-path', 'runtime/Cargo.toml'];
  command(root, 'cargo', [
    ...base,
    '--lcov',
    '--output-path',
    path.join(directory, 'rust-default.lcov'),
  ]);
  command(root, 'cargo', [
    ...base,
    '--features',
    'tree-sitter',
    '--no-clean',
    '--lcov',
    '--output-path',
    path.join(directory, 'rust.lcov'),
  ]);
  const tests = fs
    .readdirSync(path.join(root, 'npm/test'))
    .filter((file) => file.endsWith('.test.js'))
    .sort()
    .map((file) => `npm/test/${file}`);
  if (tests.length === 0) throw new Error('npm/test: no actual wrapper tests found');
  command(root, process.execPath, [
    '--experimental-test-coverage',
    '--test',
    '--test-reporter=lcov',
    `--test-reporter-destination=${path.join(directory, 'node.lcov')}`,
    ...tests,
  ]);
  const after = sourceSnapshot(root);
  if (JSON.stringify(before) !== JSON.stringify(after))
    throw new Error(
      'coverage source snapshot changed during measurement; rerun with the checkout stable',
    );
  const metadata = {
    schema_version: 1,
    metric: METRIC,
    build_modes: ['default', 'tree-sitter'],
    ...after,
    generated_at: new Date().toISOString(),
    repository_root: root,
    tools: { cargo_llvm_cov: llvm, rust: 'rustc 1.88.0', rust_full: rust, node: process.version },
    raw_sha256: Object.fromEntries(
      ['rust', 'node'].map((layer) => [
        layer,
        hash(fs.readFileSync(path.join(directory, `${layer}.lcov`))),
      ]),
    ),
  };
  fs.writeFileSync(path.join(directory, 'report.json'), JSON.stringify(metadata, null, 2) + '\n');
}

function main(args) {
  const { action, root, directory } = options(args);
  if (action === 'run') measure(root, directory);
  const result = evaluate(root, directory);
  process.stdout.write(JSON.stringify(result, null, 2) + '\n');
  if (!result.passed) {
    for (const name of ['overall', 'business_logic']) {
      const value = result[name];
      if (value.covered * 100 < value.minimum * value.mapped)
        process.stderr.write(
          `coverage ${name}: ${value.percent.toFixed(2)} percent is below ${value.minimum} percent (${value.covered}/${value.mapped} mapped lines)\n`,
        );
    }
    process.exitCode = 1;
  }
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`coverage report.json: ${error.message}\n`);
    process.exitCode = 1;
  }
}
