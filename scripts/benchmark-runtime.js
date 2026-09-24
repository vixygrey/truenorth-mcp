'use strict';

const { execFileSync, spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { startSession } = require('./lib/mcp-session.js');
const {
  characterCount,
  estimateTokens,
  roundMetrics,
  summarize,
  utf8Bytes,
} = require('./lib/benchmark-stats.js');

const REPO_ROOT = path.resolve(__dirname, '..');
const DEFAULTS = Object.freeze({
  startupSamples: 20,
  requestWarmups: 5,
  requestSamples: 30,
  watcherSamples: 20,
  skill: 'using-truenorth',
});

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const binary = prepareBinary(options.binary);
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-runtime-benchmark-'));
  const workspace = path.join(work, 'workspace');
  fs.mkdirSync(workspace);

  try {
    initializeWorkspace(binary, workspace);
    const catalog = catalogStats(path.join(workspace, 'skills'));
    const startup = await measureStartup(binary, workspace, options.startupSamples);
    const raw = {
      startup_initialize_ms: startup.initializeMs,
      idle_rss_kib: startup.idleRssKib,
    };

    const session = await initializedSession(binary, workspace);
    try {
      raw.tools_list_ms = await measureRequest(
        session,
        'tools/list',
        {},
        options.requestWarmups,
        options.requestSamples,
      );
      raw.index_skills_ms = await measureTool(
        session,
        'index_skills',
        {},
        options.requestWarmups,
        options.requestSamples,
      );

      const tiers = {};
      for (const tier of ['full', 'reasoning', 'lean']) {
        const measured = await measureToolWithPayload(
          session,
          'get_skill',
          { name: options.skill, tier },
          options.requestWarmups,
          options.requestSamples,
        );
        raw[`get_skill_${tier}_ms`] = measured.samples;
        tiers[tier] = skillPayloadMetrics(measured.result);
      }

      raw.build_skill_graph_ms = await measureTool(
        session,
        'build_skill_graph',
        {},
        options.requestWarmups,
        options.requestSamples,
      );
      raw.verify_gate_round_trip_ms = await measureTool(
        session,
        'truenorth_verify_gate',
        { phase: 'execute' },
        options.requestWarmups,
        options.requestSamples,
      );
      raw.configured_command_baseline_ms = measureShellTrue(options.requestSamples);
      raw.watcher_response_ms = await measureWatcher(session, workspace, options.watcherSamples);

      const report = buildReport(binary, options, catalog, raw, tiers);
      const output =
        options.output ?? fs.mkdtempSync(path.join(os.tmpdir(), 'tn-runtime-results-'));
      writeReports(output, report);
      console.log(`Runtime benchmark wrote ${path.join(output, 'runtime-baseline.json')}`);
      console.log(renderSummary(report));
    } finally {
      await session.terminate();
    }
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

function parseArgs(argv) {
  const options = { ...DEFAULTS };
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!value) {
      throw new Error(`missing value for ${flag}`);
    }
    switch (flag) {
      case '--binary':
        options.binary = path.resolve(value);
        break;
      case '--output':
        options.output = path.resolve(value);
        break;
      case '--startup-samples':
        options.startupSamples = positiveInteger(flag, value);
        break;
      case '--request-warmups':
        options.requestWarmups = nonnegativeInteger(flag, value);
        break;
      case '--request-samples':
        options.requestSamples = positiveInteger(flag, value);
        break;
      case '--watcher-samples':
        options.watcherSamples = positiveInteger(flag, value);
        break;
      case '--skill':
        options.skill = value;
        break;
      default:
        throw new Error(`unknown option ${flag}`);
    }
  }
  return options;
}

function positiveInteger(flag, value) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`${flag} must be a positive integer`);
  }
  return parsed;
}

function nonnegativeInteger(flag, value) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`${flag} must be a nonnegative integer`);
  }
  return parsed;
}

function prepareBinary(explicit) {
  if (explicit) {
    assertFile(explicit, 'benchmark binary');
    return explicit;
  }
  execFileSync('cargo', ['build', '--release', '--manifest-path', 'runtime/Cargo.toml'], {
    cwd: REPO_ROOT,
    stdio: 'inherit',
  });
  const binary = path.join(REPO_ROOT, 'runtime', 'target', 'release', 'truenorth-mcp');
  assertFile(binary, 'release binary');
  return binary;
}

function assertFile(file, label) {
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) {
    throw new Error(`${label} does not exist: ${file}`);
  }
}

function initializeWorkspace(binary, workspace) {
  execFileSync(
    binary,
    ['init', '--skills-dir', path.join(REPO_ROOT, 'skills'), '--profile', 'generic'],
    {
      cwd: workspace,
      env: { ...process.env, TRUENORTH_ROOT: workspace, TRUENORTH_VERIFY_CMD: 'true' },
      stdio: 'ignore',
    },
  );
}

async function initializedSession(binary, workspace) {
  const session = startSession(binary, [], workspace, {
    env: { TRUENORTH_VERIFY_CMD: 'true', RUST_LOG: 'error' },
  });
  await session.request('initialize', initializeParams());
  session.notify('notifications/initialized', {});
  return session;
}

function initializeParams() {
  return {
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'truenorth-runtime-benchmark', version: '1.0.0' },
  };
}

async function measureStartup(binary, workspace, sampleCount) {
  const initializeMs = [];
  const idleRssKib = [];
  for (let index = 0; index < sampleCount; index += 1) {
    const started = process.hrtime.bigint();
    const session = startSession(binary, [], workspace, { env: { RUST_LOG: 'error' } });
    try {
      await session.request('initialize', initializeParams());
      initializeMs.push(elapsedMs(started));
      session.notify('notifications/initialized', {});
      await delay(100);
      idleRssKib.push(residentMemoryKib(session.pid));
      await session.close();
    } finally {
      await session.terminate();
    }
  }
  return { initializeMs, idleRssKib };
}

function residentMemoryKib(pid) {
  if (process.platform === 'linux') {
    return parseLinuxRss(fs.readFileSync(`/proc/${pid}/status`, 'utf8'), pid);
  }
  if (process.platform === 'darwin') {
    const output = execFileSync('ps', ['-o', 'rss=', '-p', String(pid)], { encoding: 'utf8' });
    return parseMacRss(output, pid);
  }
  throw new Error(`RSS measurement is unsupported on ${process.platform}`);
}

function parseLinuxRss(status, pid = 'fixture') {
  const match = /^VmRSS:\s+(\d+)\s+kB$/m.exec(status);
  if (!match) {
    throw new Error(`VmRSS is missing for process ${pid}`);
  }
  return Number(match[1]);
}

function parseMacRss(output, pid = 'fixture') {
  const value = Number(output.trim());
  if (!Number.isFinite(value) || output.trim() === '') {
    throw new Error(`could not parse RSS for process ${pid}: ${output}`);
  }
  return value;
}

async function measureRequest(session, method, params, warmups, sampleCount) {
  for (let index = 0; index < warmups; index += 1) {
    await session.request(method, params);
  }
  const samples = [];
  for (let index = 0; index < sampleCount; index += 1) {
    const started = process.hrtime.bigint();
    await session.request(method, params);
    samples.push(elapsedMs(started));
  }
  return samples;
}

async function measureTool(session, name, args, warmups, sampleCount) {
  return measureRequest(session, 'tools/call', { name, arguments: args }, warmups, sampleCount);
}

async function measureToolWithPayload(session, name, args, warmups, sampleCount) {
  for (let index = 0; index < warmups; index += 1) {
    await session.request('tools/call', { name, arguments: args });
  }
  const samples = [];
  let result;
  for (let index = 0; index < sampleCount; index += 1) {
    const started = process.hrtime.bigint();
    result = await session.request('tools/call', { name, arguments: args });
    samples.push(elapsedMs(started));
  }
  return { samples, result };
}

function skillPayloadMetrics(result) {
  const text = result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error('get_skill returned no text content');
  }
  return {
    content_bytes: utf8Bytes(text),
    response_bytes: utf8Bytes(JSON.stringify(result)),
    characters: characterCount(text),
    estimated_tokens: estimateTokens(text),
  };
}

function measureShellTrue(sampleCount) {
  const samples = [];
  for (let index = 0; index < sampleCount; index += 1) {
    const started = process.hrtime.bigint();
    const result = spawnSync('/bin/sh', ['-c', 'true'], { stdio: 'ignore' });
    if (result.status !== 0) {
      throw new Error('the configured-command baseline failed');
    }
    samples.push(elapsedMs(started));
  }
  return samples;
}

async function measureWatcher(session, workspace, sampleCount) {
  const statePath = path.join(workspace, '.agent', 'tasks', 'state.yml');
  const samples = [];
  for (let index = 0; index < sampleCount; index += 1) {
    const notification = session.waitForNotification(
      (message) =>
        message.method === 'notifications/resources/updated' &&
        message.params?.uri === 'truenorth://state',
    );
    const started = process.hrtime.bigint();
    fs.writeFileSync(statePath, `phase: discover\nbenchmark_tick: ${index}\n`);
    await notification;
    samples.push(elapsedMs(started));
  }
  return samples;
}

function catalogStats(skillsRoot) {
  let skillCount = 0;
  let skillFiles = 0;
  let inputBytes = 0;
  walk(skillsRoot, (file) => {
    if (path.basename(file) === 'SKILL.md') {
      skillCount += 1;
      skillFiles += 1;
      inputBytes += fs.statSync(file).size;
    }
  });
  return { skill_count: skillCount, skill_files: skillFiles, input_bytes: inputBytes };
}

function walk(directory, visit) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      walk(target, visit);
    } else if (entry.isFile()) {
      visit(target);
    }
  }
}

function buildReport(binary, options, catalog, raw, tiers) {
  const summaries = Object.fromEntries(
    Object.entries(raw).map(([name, samples]) => [name, summarize(samples)]),
  );
  const gateMedian = summaries.verify_gate_round_trip_ms.median;
  const commandMedian = summaries.configured_command_baseline_ms.median;
  summaries.verify_gate_estimated_runtime_overhead_ms = Math.max(0, gateMedian - commandMedian);

  const fullBytes = tiers.full.content_bytes;
  for (const metrics of Object.values(tiers)) {
    metrics.compression_ratio_to_full = metrics.content_bytes / fullBytes;
  }

  return roundMetrics({
    schema_version: 1,
    generated_at: new Date().toISOString(),
    source: {
      commit: commandOutput('git', ['rev-parse', 'HEAD']),
      runtime_version: commandOutput(binary, ['--version']),
      binary_sha256: crypto.createHash('sha256').update(fs.readFileSync(binary)).digest('hex'),
    },
    environment: {
      os: process.platform,
      os_version: os.release(),
      architecture: process.arch,
      cpu: os.cpus()[0]?.model ?? 'unknown',
      logical_cpus: os.cpus().length,
      memory_bytes: os.totalmem(),
      node_version: process.version,
      rust_version: commandOutput('rustc', ['--version']),
      build_profile: 'release',
    },
    configuration: {
      startup_samples: options.startupSamples,
      request_warmups: options.requestWarmups,
      request_samples: options.requestSamples,
      watcher_samples: options.watcherSamples,
      skill: options.skill,
      startup_definition: 'fresh process spawn to successful MCP initialize response',
      verify_overhead_definition:
        'median MCP gate round trip minus median direct /bin/sh -c true execution',
    },
    catalog,
    payloads: tiers,
    raw,
    summary: summaries,
  });
}

function commandOutput(command, args) {
  return execFileSync(command, args, { cwd: REPO_ROOT, encoding: 'utf8' }).trim();
}

function writeReports(output, report) {
  fs.mkdirSync(output, { recursive: true });
  fs.writeFileSync(
    path.join(output, 'runtime-baseline.json'),
    `${JSON.stringify(report, null, 2)}\n`,
  );
  fs.writeFileSync(path.join(output, 'README.md'), renderMarkdown(report));
}

function renderMarkdown(report) {
  const rows = [
    ['Fresh-process initialize', report.summary.startup_initialize_ms, 'ms'],
    ['Idle resident memory', report.summary.idle_rss_kib, 'KiB'],
    ['tools/list', report.summary.tools_list_ms, 'ms'],
    ['index_skills', report.summary.index_skills_ms, 'ms'],
    ['get_skill full', report.summary.get_skill_full_ms, 'ms'],
    ['get_skill reasoning', report.summary.get_skill_reasoning_ms, 'ms'],
    ['get_skill lean', report.summary.get_skill_lean_ms, 'ms'],
    ['build_skill_graph', report.summary.build_skill_graph_ms, 'ms'],
    ['verify gate round trip', report.summary.verify_gate_round_trip_ms, 'ms'],
    ['Watcher response', report.summary.watcher_response_ms, 'ms'],
  ];
  const table = rows
    .map(
      ([name, metrics, unit]) =>
        `| ${name} | ${metrics.median} | ${metrics.mean} | ${metrics.p95} | ${metrics.min} | ${metrics.max} | ${unit} |`,
    )
    .join('\n');
  const payloadRows = ['full', 'reasoning', 'lean']
    .map((tier) => {
      const value = report.payloads[tier];
      return `| ${tier} | ${value.content_bytes} | ${value.response_bytes} | ${value.estimated_tokens} | ${value.compression_ratio_to_full} |`;
    })
    .join('\n');

  return `# Runtime performance baseline\n\nInformational, non-gating measurements for TrueNorth-MCP. Fresh-process startup does not clear operating-system filesystem caches. Raw samples and full environment metadata are in [runtime-baseline.json](runtime-baseline.json).\n\n## Environment\n\n- Commit: \`${report.source.commit}\`\n- Runtime: \`${report.source.runtime_version}\`\n- Platform: \`${report.environment.os} ${report.environment.architecture} ${report.environment.os_version}\`\n- CPU: ${report.environment.cpu}\n- Logical CPUs: ${report.environment.logical_cpus}\n- Memory: ${report.environment.memory_bytes} bytes\n- Node: \`${report.environment.node_version}\`\n- Rust: \`${report.environment.rust_version}\`\n- Build profile: release\n\n## Timing summary\n\n| Operation | Median | Mean | p95 | Min | Max | Unit |\n| --- | ---: | ---: | ---: | ---: | ---: | --- |\n${table}\n\nEstimated verify-gate runtime overhead, excluding the calibrated \`/bin/sh -c true\` command: **${report.summary.verify_gate_estimated_runtime_overhead_ms} ms**.\n\n## Skill payloads\n\nCanonical skill: \`${report.configuration.skill}\`. Token estimates use \`ceil(Unicode characters / 4)\`.\n\n| Tier | Content bytes | MCP result bytes | Estimated tokens | Ratio to full |\n| --- | ---: | ---: | ---: | ---: |\n${payloadRows}\n\n## Catalog input\n\n- Skills: ${report.catalog.skill_count}\n- Skill files: ${report.catalog.skill_files}\n- Input bytes: ${report.catalog.input_bytes}\n\n## Reproduce\n\n\`\`\`bash\nnode scripts/benchmark-runtime.js --output benchmarks\n\`\`\`\n\nThe suite records raw samples and spread. It defines no regression threshold. Run measurements on an otherwise idle machine and compare only equivalent environments.\n`;
}

function renderSummary(report) {
  return [
    `initialize median: ${report.summary.startup_initialize_ms.median} ms`,
    `idle RSS: ${report.summary.idle_rss_kib.median} KiB`,
    `graph median: ${report.summary.build_skill_graph_ms.median} ms`,
    `watcher median: ${report.summary.watcher_response_ms.median} ms`,
  ].join('\n');
}

function elapsedMs(started) {
  return Number(process.hrtime.bigint() - started) / 1_000_000;
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

if (require.main === module) {
  main().catch((error) => {
    console.error(`Runtime benchmark failed: ${error.message}`);
    process.exitCode = 1;
  });
}

module.exports = { buildReport, parseArgs, parseLinuxRss, parseMacRss, renderMarkdown };
