#!/usr/bin/env node
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const SCHEMA_VERSION = 1;
const DEFAULT_TIMEOUT_MS = 15_000;
const SAFE_ENV_NAMES = ['PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'SystemRoot'];

class UnavailableError extends Error {}

function stableStringify(value) {
  return `${JSON.stringify(sortValue(value), null, 2)}\n`;
}

function sortValue(value) {
  if (Array.isArray(value)) return value.map(sortValue);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, sortValue(value[key])]),
  );
}

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function treeDigest(root) {
  const entries = [];
  walk(root, '', entries);
  return sha256(stableStringify(entries));
}

function walk(root, relative, entries) {
  const directory = path.join(root, relative);
  for (const entry of fs
    .readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    const child = relative ? `${relative}/${entry.name}` : entry.name;
    const absolute = path.join(root, child);
    if (entry.isSymbolicLink()) throw new Error(`fixture contains unsupported symlink: ${child}`);
    if (entry.isDirectory()) walk(root, child, entries);
    else if (entry.isFile())
      entries.push({ path: child, sha256: sha256(fs.readFileSync(absolute)) });
    else throw new Error(`fixture contains unsupported entry: ${child}`);
  }
}

function readJson(file, label) {
  let source;
  try {
    source = fs.readFileSync(file, 'utf8');
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
  try {
    return JSON.parse(source);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (
      !['--definition', '--skill', '--output', '--check-baseline', '--update-baseline'].includes(
        argument,
      )
    ) {
      throw new Error(`unknown argument: ${argument}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`${argument} requires a path`);
    const key = argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    options[key] = path.resolve(value);
    index += 1;
  }
  if (!options.definition) throw new Error('--definition is required');
  if (!options.skill) throw new Error('--skill is required');
  if (options.checkBaseline && options.updateBaseline) {
    throw new Error('--check-baseline and --update-baseline are mutually exclusive');
  }
  return options;
}

function validateDefinition(definition, definitionPath) {
  if (!definition || typeof definition !== 'object' || Array.isArray(definition)) {
    throw new Error('benchmark definition must be an object');
  }
  if (definition.schema_version !== SCHEMA_VERSION) {
    throw new Error(`benchmark definition schema_version must be ${SCHEMA_VERSION}`);
  }
  if (typeof definition.skill !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(definition.skill)) {
    throw new Error('benchmark definition skill must be kebab-case');
  }
  if (!Number.isInteger(definition.runs) || definition.runs < 1 || definition.runs > 100) {
    throw new Error('benchmark definition runs must be an integer from 1 to 100');
  }
  if (
    definition.timeout_ms !== undefined &&
    (!Number.isInteger(definition.timeout_ms) ||
      definition.timeout_ms < 100 ||
      definition.timeout_ms > 120_000)
  ) {
    throw new Error('benchmark definition timeout_ms must be an integer from 100 to 120000');
  }
  if (!Array.isArray(definition.scenarios) || definition.scenarios.length === 0) {
    throw new Error('benchmark definition scenarios must be a non-empty array');
  }

  const directory = path.dirname(definitionPath);
  const ids = new Set();
  const splits = new Set();
  for (const scenario of definition.scenarios) {
    if (!scenario || typeof scenario !== 'object' || Array.isArray(scenario)) {
      throw new Error('each benchmark scenario must be an object');
    }
    if (typeof scenario.id !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(scenario.id)) {
      throw new Error('scenario id must be kebab-case');
    }
    if (ids.has(scenario.id)) throw new Error(`duplicate scenario id: ${scenario.id}`);
    ids.add(scenario.id);
    if (!['train', 'validation'].includes(scenario.split)) {
      throw new Error(`scenario ${scenario.id} split must be train or validation`);
    }
    splits.add(scenario.split);
    if (
      typeof scenario.weight !== 'number' ||
      !Number.isFinite(scenario.weight) ||
      scenario.weight <= 0
    ) {
      throw new Error(`scenario ${scenario.id} weight must be a positive number`);
    }
    if (typeof scenario.fixture !== 'string' || !scenario.fixture) {
      throw new Error(`scenario ${scenario.id} fixture must be a non-empty path`);
    }
    const fixture = path.resolve(directory, scenario.fixture);
    if (fixture !== directory && !fixture.startsWith(`${directory}${path.sep}`)) {
      throw new Error(`scenario ${scenario.id} fixture escapes definition directory`);
    }
    if (!fs.statSync(fixture, { throwIfNoEntry: false })?.isDirectory()) {
      throw new Error(`scenario ${scenario.id} fixture is not a directory`);
    }
    validateCommand(scenario.executor, `scenario ${scenario.id} executor`);
    validateCommand(scenario.grader, `scenario ${scenario.id} grader`);
  }
  for (const split of ['train', 'validation']) {
    if (!splits.has(split)) throw new Error(`benchmark definition needs a ${split} scenario`);
  }
  return definition;
}

function validateCommand(command, label) {
  if (!command || typeof command !== 'object' || Array.isArray(command)) {
    throw new Error(`${label} must be an object`);
  }
  if (typeof command.command !== 'string' || !command.command || /[\r\n\0]/.test(command.command)) {
    throw new Error(`${label}.command must be a non-empty executable name`);
  }
  if (
    !Array.isArray(command.args) ||
    command.args.some((argument) => typeof argument !== 'string')
  ) {
    throw new Error(`${label}.args must be an array of strings`);
  }
}

function commandEnvironment(extra) {
  const environment = {};
  for (const name of SAFE_ENV_NAMES) {
    if (process.env[name] !== undefined) environment[name] = process.env[name];
  }
  return { ...environment, ...extra };
}

function execute(command, cwd, environment, timeoutMs) {
  const result = spawnSync(command.command, command.args, {
    cwd,
    encoding: 'utf8',
    env: environment,
    shell: false,
    timeout: timeoutMs,
    maxBuffer: 1024 * 1024,
  });
  return {
    exit_code: result.status,
    signal: result.signal,
    timed_out: result.error?.code === 'ETIMEDOUT',
    spawn_error: result.error && result.error.code !== 'ETIMEDOUT' ? result.error.message : null,
    stdout: result.stdout || '',
    stderr: result.stderr || '',
  };
}

function publicResult(result) {
  return {
    exit_code: result.exit_code,
    signal: result.signal,
    timed_out: result.timed_out,
    spawn_error: result.spawn_error,
  };
}

function runOne(definitionPath, skillPath, scenario, mode, runNumber, timeoutMs) {
  const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'truenorth-skill-benchmark-'));
  try {
    const workspace = path.join(temporaryRoot, 'workspace');
    const fixture = path.resolve(path.dirname(definitionPath), scenario.fixture);
    fs.cpSync(fixture, workspace, { recursive: true, verbatimSymlinks: true });

    let contextSkillPath = '';
    if (mode === 'with_skill') {
      const contextDirectory = path.join(workspace, '.benchmark-context');
      fs.mkdirSync(contextDirectory, { recursive: true });
      contextSkillPath = path.join(contextDirectory, 'SKILL.md');
      fs.copyFileSync(skillPath, contextSkillPath);
    }

    const resultDirectory = path.join(workspace, '.benchmark-result');
    fs.mkdirSync(resultDirectory, { recursive: true });
    const stdoutPath = path.join(resultDirectory, 'executor-stdout.txt');
    const stderrPath = path.join(resultDirectory, 'executor-stderr.txt');
    const homeDirectory = path.join(workspace, '.benchmark-home');
    const temporaryDirectory = path.join(workspace, '.benchmark-tmp');
    fs.mkdirSync(homeDirectory);
    fs.mkdirSync(temporaryDirectory);
    const environment = commandEnvironment({
      HOME: homeDirectory,
      TMPDIR: temporaryDirectory,
      TMP: temporaryDirectory,
      TEMP: temporaryDirectory,
      TRUENORTH_BENCHMARK_MODE: mode,
      TRUENORTH_BENCHMARK_RUN: String(runNumber),
      TRUENORTH_BENCHMARK_WORKSPACE: workspace,
      TRUENORTH_BENCHMARK_SKILL_PATH: contextSkillPath,
      TRUENORTH_BENCHMARK_STDOUT: stdoutPath,
      TRUENORTH_BENCHMARK_STDERR: stderrPath,
    });
    const executor = execute(scenario.executor, workspace, environment, timeoutMs);
    fs.writeFileSync(stdoutPath, executor.stdout);
    fs.writeFileSync(stderrPath, executor.stderr);
    const grader =
      executor.exit_code === 0 && !executor.timed_out && !executor.spawn_error
        ? execute(scenario.grader, workspace, environment, timeoutMs)
        : {
            exit_code: null,
            signal: null,
            timed_out: false,
            spawn_error: 'executor did not complete',
            stdout: '',
            stderr: '',
          };
    return {
      passed:
        executor.exit_code === 0 &&
        !executor.timed_out &&
        !executor.spawn_error &&
        grader.exit_code === 0 &&
        !grader.timed_out &&
        !grader.spawn_error,
      executor: publicResult(executor),
      grader: publicResult(grader),
    };
  } finally {
    fs.rmSync(temporaryRoot, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
}

function round(value) {
  const rounded = Math.round((value + Number.EPSILON) * 100) / 100;
  return Object.is(rounded, -0) ? 0 : rounded;
}

function aggregate(scenarios, split) {
  const selected = scenarios.filter((scenario) => scenario.split === split);
  const totalWeight = selected.reduce((sum, scenario) => sum + scenario.weight, 0);
  const withSkill =
    selected.reduce((sum, scenario) => sum + scenario.weight * scenario.with_pass_rate, 0) /
    totalWeight;
  const withoutSkill =
    selected.reduce((sum, scenario) => sum + scenario.weight * scenario.without_pass_rate, 0) /
    totalWeight;
  return {
    with_skill: round(withSkill),
    without_skill: round(withoutSkill),
    delta: round(withSkill - withoutSkill),
    scenario_ids: selected.map((scenario) => scenario.id),
  };
}

function buildReport(definition, definitionPath, skillPath) {
  const timeoutMs = definition.timeout_ms ?? DEFAULT_TIMEOUT_MS;
  const scenarios = definition.scenarios.map((scenario) => {
    const fixtureSha256 = treeDigest(path.resolve(path.dirname(definitionPath), scenario.fixture));
    const runs = { with_skill: [], without_skill: [] };
    for (const mode of ['with_skill', 'without_skill']) {
      for (let run = 1; run <= definition.runs; run += 1) {
        runs[mode].push(runOne(definitionPath, skillPath, scenario, mode, run, timeoutMs));
      }
    }
    const withPassRate = runs.with_skill.filter((result) => result.passed).length / definition.runs;
    const withoutPassRate =
      runs.without_skill.filter((result) => result.passed).length / definition.runs;
    return {
      id: scenario.id,
      split: scenario.split,
      weight: scenario.weight,
      fixture_sha256: fixtureSha256,
      with_pass_rate: round(withPassRate),
      without_pass_rate: round(withoutPassRate),
      delta: round(withPassRate - withoutPassRate),
      runs,
    };
  });
  return {
    schema_version: SCHEMA_VERSION,
    skill: definition.skill,
    definition_sha256: sha256(fs.readFileSync(definitionPath)),
    skill_sha256: sha256(fs.readFileSync(skillPath)),
    runs_per_scenario: definition.runs,
    train: aggregate(scenarios, 'train'),
    validation: aggregate(scenarios, 'validation'),
    scenarios,
  };
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, stableStringify(value));
}

function checkBaseline(report, baselinePath) {
  if (!fs.existsSync(baselinePath)) {
    throw new UnavailableError(`baseline report not found: ${baselinePath}`);
  }
  const baseline = readJson(baselinePath, 'baseline report');
  if (baseline.schema_version !== SCHEMA_VERSION || baseline.skill !== report.skill) {
    throw new Error('baseline report schema or skill does not match the current report');
  }
  if (baseline.definition_sha256 !== report.definition_sha256) {
    throw new Error('baseline report definition does not match the current definition');
  }
  if (typeof baseline.validation?.delta !== 'number') {
    throw new Error('baseline report validation.delta must be a number');
  }
  if (report.validation.delta < baseline.validation.delta) {
    throw new Error(
      `validation delta regressed from ${baseline.validation.delta} to ${report.validation.delta}`,
    );
  }
}

function main(argv) {
  const options = parseArgs(argv);
  if (!fs.existsSync(options.definition)) {
    throw new UnavailableError(`benchmark definition not found: ${options.definition}`);
  }
  if (!fs.statSync(options.skill, { throwIfNoEntry: false })?.isFile()) {
    throw new UnavailableError(`skill file not found: ${options.skill}`);
  }
  const definition = validateDefinition(
    readJson(options.definition, 'benchmark definition'),
    options.definition,
  );
  const report = buildReport(definition, options.definition, options.skill);
  if (options.output) writeJson(options.output, report);
  if (options.checkBaseline) checkBaseline(report, options.checkBaseline);
  if (options.updateBaseline) writeJson(options.updateBaseline, report);
  process.stdout.write(stableStringify(report));
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    if (error instanceof UnavailableError) {
      process.stderr.write(`TRUENORTH_BENCHMARK_UNAVAILABLE: ${error.message}\n`);
      process.exitCode = 2;
    } else {
      process.stderr.write(`run-benchmark: ${error.message}\n`);
      process.exitCode = 1;
    }
  }
}

module.exports = {
  SCHEMA_VERSION,
  aggregate,
  buildReport,
  parseArgs,
  stableStringify,
  validateDefinition,
};
