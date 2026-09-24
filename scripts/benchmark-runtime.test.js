'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const { parseArgs, parseLinuxRss, parseMacRss, renderMarkdown } = require('./benchmark-runtime.js');
const {
  characterCount,
  estimateTokens,
  roundMetrics,
  summarize,
  utf8Bytes,
} = require('./lib/benchmark-stats.js');

test('summarize reports spread and nearest-rank p95', () => {
  assert.deepEqual(summarize([5, 1, 2, 3, 4]), {
    min: 1,
    median: 3,
    mean: 3,
    stddev: Math.sqrt(2),
    p95: 5,
    max: 5,
  });
});

test('summarize rejects an empty sample set', () => {
  assert.throws(() => summarize([]), /at least one benchmark sample/);
});

test('payload accounting separates UTF-8 bytes from Unicode characters', () => {
  assert.equal(utf8Bytes('é🙂'), 6);
  assert.equal(characterCount('é🙂'), 2);
  assert.equal(estimateTokens('12345'), 2);
});

test('RSS parsers accept platform output and reject missing values', () => {
  assert.equal(parseLinuxRss('Name:\tfixture\nVmRSS:\t  12345 kB\n'), 12345);
  assert.equal(parseMacRss('  6789\n'), 6789);
  assert.throws(() => parseLinuxRss('Name:\tfixture\n'), /VmRSS is missing/);
  assert.throws(() => parseMacRss(''), /could not parse RSS/);
});

test('roundMetrics gives reports stable millisecond precision', () => {
  assert.deepEqual(roundMetrics({ value: 1.23456, raw: [2.34567] }), {
    value: 1.235,
    raw: [2.346],
  });
});

test('parseArgs accepts reduced smoke sample counts and rejects zero samples', () => {
  assert.deepEqual(
    parseArgs([
      '--startup-samples',
      '2',
      '--request-warmups',
      '0',
      '--request-samples',
      '3',
      '--watcher-samples',
      '2',
      '--skill',
      'verify-work',
    ]),
    {
      startupSamples: 2,
      requestWarmups: 0,
      requestSamples: 3,
      watcherSamples: 2,
      skill: 'verify-work',
    },
  );
  assert.throws(() => parseArgs(['--request-samples', '0']), /positive integer/);
});

test('renderMarkdown publishes environment, spread, payloads, and non-gating policy', () => {
  const metrics = { min: 1, median: 2, mean: 2, stddev: 1, p95: 3, max: 3 };
  const report = {
    source: { commit: 'abc123', runtime_version: 'truenorth-mcp 1.0.1' },
    environment: {
      os: 'linux',
      architecture: 'x64',
      os_version: 'test',
      cpu: 'Fixture CPU',
      logical_cpus: 4,
      memory_bytes: 1024,
      node_version: 'v22.0.0',
      rust_version: 'rustc test',
    },
    configuration: { skill: 'using-truenorth' },
    catalog: { skill_count: 2, skill_files: 2, input_bytes: 100 },
    payloads: {
      full: payload(100, 1),
      reasoning: payload(80, 0.8),
      lean: payload(50, 0.5),
    },
    summary: {
      startup_initialize_ms: metrics,
      idle_rss_kib: metrics,
      tools_list_ms: metrics,
      index_skills_ms: metrics,
      get_skill_full_ms: metrics,
      get_skill_reasoning_ms: metrics,
      get_skill_lean_ms: metrics,
      build_skill_graph_ms: metrics,
      verify_gate_round_trip_ms: metrics,
      watcher_response_ms: metrics,
      verify_gate_estimated_runtime_overhead_ms: 1,
    },
  };

  const markdown = renderMarkdown(report);
  assert.match(markdown, /Informational, non-gating/);
  assert.match(markdown, /Fixture CPU/);
  assert.match(markdown, /\| Operation \| Median \| Mean \| p95/);
  assert.match(markdown, /Estimated verify-gate runtime overhead/);
  assert.match(markdown, /\| lean \| 50 \| 60 \| 13 \| 0.5 \|/);
  assert.match(markdown, /defines no regression threshold/);
});

function payload(contentBytes, ratio) {
  return {
    content_bytes: contentBytes,
    response_bytes: contentBytes + 10,
    estimated_tokens: Math.ceil(contentBytes / 4),
    compression_ratio_to_full: ratio,
  };
}
