'use strict';

const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');

const METRIC = 'native-mapped-lines-v1';
const INLINE_TEST_MODULES = new Set([
  'runtime/src/config/gate_environment.rs',
  'runtime/src/engine/backlog.rs',
  'runtime/src/engine/digest.rs',
  'runtime/src/engine/regex_util.rs',
  'runtime/src/tools/receipt.rs',
  'runtime/src/tools/result.rs',
]);
const WRAPPER_SOURCES = ['npm/lib/runner.js', 'npm/bin/truenorth.js'];
const hash = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');

function sourceSnapshot(root) {
  const files = [
    '.agent/config/rules.yml',
    'runtime/Cargo.toml',
    'runtime/Cargo.lock',
    'rust-toolchain.toml',
    '.node-version',
    'package.json',
    'package-lock.json',
    'npm/package.json',
  ];
  function walk(relative) {
    for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
      const file = `${relative}/${entry.name}`;
      if (entry.isSymbolicLink()) throw new Error(`source ${file}: symbolic links are unsupported`);
      if (entry.isDirectory()) walk(file);
      else if (entry.isFile()) files.push(file);
    }
  }
  for (const directory of [
    'runtime/src',
    'runtime/tests',
    'npm/lib',
    'npm/bin',
    'npm/test',
    'scripts',
    '.github/workflows',
  ])
    walk(directory);
  const digest = crypto.createHash('sha256');
  for (const file of files.sort()) {
    digest
      .update(file)
      .update('\0')
      .update(fs.readFileSync(path.join(root, file)))
      .update('\0');
  }
  return {
    head: execFileSync('git', ['rev-parse', '--verify', 'HEAD'], {
      cwd: root,
      encoding: 'utf8',
      env: { PATH: process.env.PATH },
    }).trim(),
    source_sha256: digest.digest('hex'),
  };
}

function readPolicy(root) {
  const rows = fs.readFileSync(path.join(root, '.agent/config/rules.yml'), 'utf8').split('\n');
  function mapping(input, key, indent) {
    const heading = ' '.repeat(indent) + key + ':';
    const starts = input.flatMap((row, index) =>
      row.replace(/\s+#.*$/, '').trimEnd() === heading ? [index] : [],
    );
    if (starts.length !== 1)
      throw new Error(`coverage policy ${key}: expected one indented YAML mapping`);
    const start = starts[0] + 1;
    let end = start;
    while (end < input.length) {
      const row = input[end];
      if (row.trim() && !row.trimStart().startsWith('#') && row.search(/\S/) <= indent) break;
      end++;
    }
    return input.slice(start, end);
  }
  const coverage = mapping(
    mapping(mapping(rows, 'advisory', 0), 'quality_gates', 2),
    'coverage',
    4,
  ).join('\n');
  const policy = {};
  for (const [field, expected] of [
    ['overall_minimum_percent', 80],
    ['business_logic_minimum_percent', 95],
  ]) {
    const matches = [
      ...coverage.matchAll(
        new RegExp(`^ {6}${field}:\\s*([0-9]+(?:\\.[0-9]+)?)\\s*(?:#.*)?$`, 'gm'),
      ),
    ];
    if (matches.length !== 1 || Number(matches[0][1]) !== expected) {
      throw new Error(
        `coverage policy ${field}: expected the approved ${expected} percent threshold`,
      );
    }
    policy[field] = expected;
  }
  return policy;
}

function parseLcov(text, reportRoot, label, layer) {
  const files = new Map();
  let record;
  function fail(message) {
    throw new Error(`${label}: malformed LCOV: ${message}`);
  }
  for (const row of text.split(/\r?\n/)) {
    if (!row) continue;
    if (row.startsWith('SF:')) {
      if (record) fail('source record missing end_of_record');
      const name = path
        .relative(reportRoot, path.resolve(reportRoot, row.slice(3)))
        .split(path.sep)
        .join('/');
      if (!name || name.startsWith('../') || files.has(name))
        fail(`invalid or duplicate source ${row.slice(3)}`);
      record = { name, lines: new Map(), found: null, hit: null };
    } else if (row.startsWith('DA:')) {
      const match = /^DA:([1-9][0-9]*),([0-9]+)(?:,[^,]+)?$/.exec(row);
      if (!record || !match) fail(`invalid line record ${row}`);
      const line = Number(match[1]);
      const count = Number(match[2]);
      if (!Number.isSafeInteger(line) || !Number.isSafeInteger(count) || record.lines.has(line))
        fail(`invalid or duplicate line ${row}`);
      record.lines.set(line, count);
    } else if (row.startsWith('LF:') || row.startsWith('LH:')) {
      if (!record || !/^(LF|LH):[0-9]+$/.test(row)) fail(`invalid summary ${row}`);
      const field = row.startsWith('LF:') ? 'found' : 'hit';
      if (record[field] !== null) fail(`duplicate summary ${row}`);
      record[field] = Number(row.slice(3));
    } else if (row === 'end_of_record') {
      if (!record) fail('end_of_record without source');
      const hit = [...record.lines.values()].filter((count) => count > 0).length;
      if (
        !Number.isSafeInteger(record.found) ||
        !Number.isSafeInteger(record.hit) ||
        record.hit > record.found
      )
        fail(`invalid line summary for ${record.name}`);
      // LLVM LF/LH can differ from unique DA records; DA defines our declared metric.
      // Node's V8 exporter uses the same unique-line accounting for DA and its summary.
      if (layer === 'node' && (record.found !== record.lines.size || record.hit !== hit))
        fail(`line summary disagrees with DA records for ${record.name}`);
      files.set(record.name, record.lines);
      record = undefined;
    } else if (row.startsWith('TN:')) {
      if (record) fail('test-name record inside an unfinished source record');
    } else {
      const validOptional =
        /^FN:[0-9]+(?:,[0-9]+)?,.+$/.test(row) ||
        /^FNDA:[0-9]+,.+$/.test(row) ||
        /^(FNF|FNH|BRF|BRH):[0-9]+$/.test(row) ||
        /^BRDA:[0-9]+,[0-9]+,[0-9]+,([0-9]+|-)$/.test(row);
      if (!record || !validOptional) fail(`invalid or unknown record ${row}`);
    }
  }
  if (record || files.size === 0) fail('empty or incomplete report');
  return files;
}

function productionLines(root, name, lines) {
  const text = fs.readFileSync(path.join(root, name), 'utf8');
  const source = text.split('\n');
  const maximum = source.length - (source.at(-1) === '' ? 1 : 0);
  let excludedStart = Infinity;
  let excludedEnd = Infinity;
  if (INLINE_TEST_MODULES.has(name)) {
    const marker = source.findIndex(
      (line, index) => line === '#[cfg(test)]' && source[index + 1] === 'mod tests {',
    );
    if (marker < 0) throw new Error(`source ${name}: expected the declared inline test module`);
    excludedStart = marker + 1;
  } else if (name === 'runtime/src/server.rs') {
    const marker = source.findIndex(
      (line, index) => line === '#[cfg(test)]' && source[index + 1] === 'impl TrueNorthServer {',
    );
    if (marker < 0)
      throw new Error(`source ${name}: expected the declared test-only server implementation`);
    excludedStart = marker + 1;
    excludedEnd = source.findIndex((line, index) => index > marker && line === '}') + 1;
    if (excludedEnd <= excludedStart)
      throw new Error(`source ${name}: incomplete test-only implementation`);
  }
  const result = new Map();
  for (const [line, count] of lines) {
    if (line > maximum)
      throw new Error(`source ${name}: reported line ${line} exceeds ${maximum} source lines`);
    if (line < excludedStart || line > excludedEnd) result.set(line, count);
  }
  return result;
}

function totals(files) {
  let covered = 0;
  let mapped = 0;
  for (const lines of files.values())
    for (const count of lines.values()) {
      mapped++;
      if (count > 0) covered++;
    }
  if (mapped === 0) throw new Error('coverage source set: no mapped production lines');
  return { covered, mapped, percent: (covered * 100) / mapped };
}

function evaluate(root, directory, now = Date.now()) {
  const policy = readPolicy(root);
  const metadata = JSON.parse(fs.readFileSync(path.join(directory, 'report.json'), 'utf8'));
  if (
    metadata.schema_version !== 1 ||
    metadata.metric !== METRIC ||
    JSON.stringify(metadata.build_modes) !== '["default","tree-sitter"]'
  ) {
    throw new Error(
      'report.json schema/metric/build_modes: expected native-mapped-lines-v1 with default and tree-sitter',
    );
  }
  const generated = Date.parse(metadata.generated_at);
  if (!Number.isFinite(generated) || generated > now + 60000 || now - generated > 86400000)
    throw new Error(
      'report.json generated_at: stale or invalid; regenerate coverage within 24 hours',
    );
  const snapshot = sourceSnapshot(root);
  for (const field of ['head', 'source_sha256'])
    if (metadata[field] !== snapshot[field])
      throw new Error(`report.json ${field}: stale; regenerate coverage for the current checkout`);
  if (
    metadata.tools?.cargo_llvm_cov !== 'cargo-llvm-cov 0.9.1' ||
    metadata.tools?.rust !== 'rustc 1.88.0' ||
    !/^v(2[2-9]|[3-9][0-9])\./.test(metadata.tools?.node || '')
  ) {
    throw new Error(
      'report.json tools: expected cargo-llvm-cov 0.9.1, Rust 1.88.0, and built-in Node coverage on Node 22 or newer',
    );
  }
  if (typeof metadata.repository_root !== 'string' || !path.isAbsolute(metadata.repository_root))
    throw new Error('report.json repository_root: expected the absolute measurement root');
  const reports = {};
  for (const layer of ['rust', 'node']) {
    const filename = `${layer}.lcov`;
    const bytes = fs.readFileSync(path.join(directory, filename));
    if (hash(bytes) !== metadata.raw_sha256?.[layer])
      throw new Error(`${filename} raw_sha256: missing or changed report digest`);
    reports[layer] = parseLcov(bytes.toString('utf8'), metadata.repository_root, filename, layer);
  }
  const rust = new Map();
  const business = new Map();
  for (const [name, lines] of reports.rust) {
    if (
      !name.startsWith('runtime/src/') ||
      name.endsWith('_tests.rs') ||
      name.startsWith('runtime/src/bin/')
    )
      continue;
    const selected = productionLines(root, name, lines);
    rust.set(name, selected);
    if (name === 'runtime/src/config.rs' || /^runtime\/src\/(config|engine|tools)\//.test(name))
      business.set(name, selected);
  }
  const node = new Map();
  for (const name of WRAPPER_SOURCES) {
    let lines = reports.node.get(name);
    if (!lines) {
      const text = fs.readFileSync(path.join(root, name), 'utf8');
      const count = text.split('\n').length - (text.endsWith('\n') ? 1 : 0);
      lines = new Map(Array.from({ length: count }, (_, index) => [index + 1, 0]));
    }
    node.set(name, productionLines(root, name, lines));
  }
  const result = {
    schema_version: 1,
    metric: METRIC,
    ...snapshot,
    rust: totals(rust),
    node: totals(node),
    overall: totals(new Map([...rust, ...node])),
    business_logic: totals(business),
  };
  result.overall.minimum = policy.overall_minimum_percent;
  result.business_logic.minimum = policy.business_logic_minimum_percent;
  result.passed =
    result.overall.covered * 100 >= result.overall.minimum * result.overall.mapped &&
    result.business_logic.covered * 100 >=
      result.business_logic.minimum * result.business_logic.mapped;
  return result;
}

module.exports = { METRIC, hash, sourceSnapshot, evaluate };
