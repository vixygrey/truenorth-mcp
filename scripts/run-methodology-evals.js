#!/usr/bin/env node
'use strict';

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const {
  REPORT_SCHEMA_VERSION,
  baselineProjection,
  definitionDigests,
  diffSnapshots,
  readJson,
  runProcess,
  sha256,
  snapshotTree,
  stableStringify,
  temporaryRoot,
  validateManifest,
} = require('./lib/methodology-eval.js');
const { configuration: modelConfiguration } = require('./lib/methodology-model-adapter.js');

const REPO_ROOT = path.resolve(__dirname, '..');
const SUITE_ROOT = path.join(REPO_ROOT, 'evals', 'methodology');
const SOURCE_EXCLUDES = ['.git', '.agent/runtime', 'node_modules', 'runtime/target'];

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const manifest = readJson(path.join(SUITE_ROOT, 'manifest.json'));
  const entries = validateManifest(manifest, SUITE_ROOT)
    .filter((entry) => entry.scenario.modes.includes(options.mode))
    .filter((entry) => !options.scenarios || options.scenarios.has(entry.scenario.id))
    .sort((left, right) => left.scenario.id.localeCompare(right.scenario.id));
  if (entries.length === 0) throw new Error('no scenarios matched the requested mode and filters');

  const binary = await resolveBinary(options.binary);
  const sourceBefore = snapshotTree(REPO_ROOT, { exclude: SOURCE_EXCLUDES });
  const model = modelConfiguration();
  const scenarios = [];
  for (const entry of entries) {
    scenarios.push(await runScenario(entry, options.mode, binary, model));
  }
  const sourceAfter = snapshotTree(REPO_ROOT, { exclude: SOURCE_EXCLUDES });
  const sourceChanges = diffSnapshots(sourceBefore, sourceAfter);
  if (sourceChanges.length > 0) {
    throw new Error(
      `methodology evaluations changed the source checkout: ${sourceChanges.join(', ')}`,
    );
  }

  const report = buildReport(options.mode, binary, scenarios, model);
  if (options.checkBaseline) checkBaseline(report);
  if (options.updateBaseline) updateBaseline(report);
  process.stdout.write(stableStringify(report));
  if (report.summary.fail > 0) process.exitCode = 1;
}

async function runScenario(entry, mode, binary, model) {
  const digests = definitionDigests(entry);
  const expected = [...entry.scenario.expected_assertions[mode]].sort();
  if (mode === 'model' && !model.available) {
    return {
      id: entry.scenario.id,
      mode,
      status: 'skip',
      skip_reason: model.reason,
      definition_digests: digests,
      assertions: expected.map((id) => ({ id, passed: false, evidence: 'not run' })),
      events: [],
    };
  }

  const temp = temporaryRoot(`truenorth-methodology-${entry.scenario.id}-`);
  const inputPath = path.join(temp, 'driver-input.json');
  const evidencePath = path.join(temp, 'evidence.json');
  const gradeInputPath = path.join(temp, 'grade-input.json');
  const gradeOutputPath = path.join(temp, 'grade-output.json');
  try {
    fs.writeFileSync(
      inputPath,
      stableStringify({
        binary,
        mode,
        repo_root: REPO_ROOT,
        scenario: entry.scenario,
        scenario_directory: entry.directory,
        scenario_root: path.join(temp, 'run'),
      }),
    );
    fs.mkdirSync(path.join(temp, 'run'));
    const driver = await runProcess(
      process.execPath,
      [path.join(__dirname, 'lib', 'methodology-driver-child.js'), inputPath, evidencePath],
      {
        cwd: REPO_ROOT,
        env: process.env,
        timeoutMs: entry.scenario.timeout_ms,
        label: `${entry.scenario.id} driver`,
      },
    );
    if (!fs.existsSync(evidencePath)) {
      return failedScenario(
        entry,
        mode,
        digests,
        expected,
        driver.stderr || 'driver produced no evidence',
      );
    }
    const evidence = readJson(evidencePath);
    if (driver.code !== 0 && evidence.driver_error) {
      return failedScenario(
        entry,
        mode,
        digests,
        expected,
        `driver failed: ${evidence.driver_error.message}`,
      );
    }
    const workspaceBeforeGrade = snapshotTree(evidence.scenario_root, {
      exclude: ['workspace/.agent/runtime', 'workspace/.git'],
    });
    fs.writeFileSync(gradeInputPath, stableStringify({ evidence, mode, scenario: entry.scenario }));
    const grader = await runProcess(
      process.execPath,
      [
        path.join(__dirname, 'lib', 'methodology-grader-child.js'),
        gradeInputPath,
        path.join(entry.directory, entry.scenario.grader),
        gradeOutputPath,
      ],
      {
        cwd: REPO_ROOT,
        timeoutMs: entry.scenario.grader_timeout_ms ?? entry.scenario.timeout_ms,
        label: `${entry.scenario.id} grader`,
      },
    );
    if (grader.code !== 0 || !fs.existsSync(gradeOutputPath)) {
      return failedScenario(entry, mode, digests, expected, grader.stderr || 'grader failed');
    }
    const workspaceAfterGrade = snapshotTree(evidence.scenario_root, {
      exclude: ['workspace/.agent/runtime', 'workspace/.git'],
    });
    const graderChanges = diffSnapshots(workspaceBeforeGrade, workspaceAfterGrade);
    if (graderChanges.length > 0) {
      return failedScenario(
        entry,
        mode,
        digests,
        expected,
        `grader mutated scenario paths: ${graderChanges.join(', ')}`,
      );
    }
    const assertions = validateAssertions(
      entry.scenario.id,
      readJson(gradeOutputPath).assertions,
      expected,
    );
    return {
      id: entry.scenario.id,
      mode,
      status: assertions.every((assertion) => assertion.passed) ? 'pass' : 'fail',
      skip_reason: null,
      definition_digests: digests,
      assertions,
      events: evidence.events,
    };
  } finally {
    fs.rmSync(temp, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  }
}

function failedScenario(entry, mode, digests, expected, evidence) {
  return {
    id: entry.scenario.id,
    mode,
    status: 'fail',
    skip_reason: null,
    definition_digests: digests,
    assertions: expected.map((id) => ({ id, passed: false, evidence })),
    events: [],
  };
}

function validateAssertions(scenario, assertions, expected) {
  if (!Array.isArray(assertions)) throw new Error(`${scenario} grader returned no assertions`);
  const normalized = assertions
    .map((assertion) => {
      if (
        !assertion ||
        typeof assertion.id !== 'string' ||
        typeof assertion.passed !== 'boolean' ||
        typeof assertion.evidence !== 'string'
      ) {
        throw new Error(`${scenario} grader returned an invalid assertion`);
      }
      return { id: assertion.id, passed: assertion.passed, evidence: assertion.evidence };
    })
    .sort((left, right) => left.id.localeCompare(right.id));
  const actual = normalized.map((assertion) => assertion.id);
  if (stableStringify(actual) !== stableStringify(expected)) {
    throw new Error(`${scenario} assertion ids differ: expected ${expected}, got ${actual}`);
  }
  return normalized;
}

function buildReport(mode, binary, scenarios, model) {
  const pass = scenarios.filter((scenario) => scenario.status === 'pass').length;
  const fail = scenarios.filter((scenario) => scenario.status === 'fail').length;
  const skip = scenarios.filter((scenario) => scenario.status === 'skip').length;
  return {
    schema_version: REPORT_SCHEMA_VERSION,
    suite_schema_version: 1,
    source: {
      commit: git(['rev-parse', 'HEAD']).trim(),
      dirty: git(['status', '--porcelain', '--untracked-files=all']).trim().length > 0,
    },
    runtime: {
      version: execFileSync(binary, ['--version'], { encoding: 'utf8' }).trim(),
      binary_sha256: sha256(fs.readFileSync(binary)),
    },
    bundle: {
      version: readJson(path.join(REPO_ROOT, 'npm', 'bundle', 'current.json')).bundle_version,
      manifest_sha256: sha256(
        fs.readFileSync(path.join(REPO_ROOT, 'npm', 'bundle', 'current.json')),
      ),
    },
    runner: { name: 'truenorth-methodology-evals', version: '1' },
    client: { name: 'scripts/lib/mcp-session.js', version: '1' },
    environment: { os: process.platform, architecture: process.arch, node: process.version },
    mode,
    model:
      mode === 'model'
        ? { provider: model.provider, id: model.model, available: model.available }
        : { provider: null, id: null, available: false },
    scenarios,
    summary: { pass, fail, skip },
  };
}

function checkBaseline(report) {
  if (report.mode !== 'deterministic')
    throw new Error('baseline checking requires deterministic mode');
  const baselinePath = path.join(SUITE_ROOT, 'baselines', 'deterministic.json');
  const expected = readJson(baselinePath);
  const actual = baselineProjection(report);
  if (stableStringify(actual) !== stableStringify(expected)) {
    throw new Error('deterministic methodology results differ from the checked-in baseline');
  }
}

function updateBaseline(report) {
  if (report.mode !== 'deterministic')
    throw new Error('baseline update requires deterministic mode');
  if (report.summary.fail > 0 || report.summary.skip > 0) {
    throw new Error('refusing to update a baseline with failed or skipped scenarios');
  }
  const baselinePath = path.join(SUITE_ROOT, 'baselines', 'deterministic.json');
  fs.writeFileSync(baselinePath, stableStringify(baselineProjection(report)));
}

async function resolveBinary(explicit) {
  if (explicit) return path.resolve(explicit);
  const build = await runProcess(
    'cargo',
    ['build', '--locked', '--manifest-path', 'runtime/Cargo.toml', '--bin', 'truenorth-mcp'],
    { cwd: REPO_ROOT, timeoutMs: 600_000, label: 'runtime build' },
  );
  if (build.code !== 0) throw new Error(`runtime build failed: ${build.stderr.trim()}`);
  return path.join(REPO_ROOT, 'runtime', 'target', 'debug', 'truenorth-mcp');
}

function parseArgs(argv) {
  const options = {
    mode: 'deterministic',
    checkBaseline: false,
    updateBaseline: false,
    scenarios: null,
    binary: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === '--check-baseline') options.checkBaseline = true;
    else if (flag === '--update-baseline') options.updateBaseline = true;
    else if (flag === '--mode') options.mode = requiredValue(argv, ++index, flag);
    else if (flag === '--scenario') {
      options.scenarios = new Set(requiredValue(argv, ++index, flag).split(',').filter(Boolean));
    } else if (flag === '--binary') options.binary = requiredValue(argv, ++index, flag);
    else throw new Error(`unknown option ${flag}`);
  }
  if (!['deterministic', 'model'].includes(options.mode)) {
    throw new Error(`unknown mode ${options.mode}`);
  }
  if (options.checkBaseline && options.updateBaseline) {
    throw new Error('--check-baseline and --update-baseline are mutually exclusive');
  }
  return options;
}

function requiredValue(argv, index, flag) {
  if (!argv[index]) throw new Error(`${flag} requires a value`);
  return argv[index];
}

function git(args) {
  return execFileSync('git', args, { cwd: REPO_ROOT, encoding: 'utf8' });
}

if (require.main === module) {
  main().catch((error) => {
    console.error(`methodology evals failed: ${error.message}`);
    process.exitCode = 1;
  });
}

module.exports = { buildReport, parseArgs, validateAssertions };
