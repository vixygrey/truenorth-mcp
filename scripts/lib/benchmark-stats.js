'use strict';

function summarize(samples) {
  if (!Array.isArray(samples) || samples.length === 0) {
    throw new Error('at least one benchmark sample is required');
  }
  const sorted = [...samples].sort((left, right) => left - right);
  const mean = samples.reduce((sum, value) => sum + value, 0) / samples.length;
  const variance = samples.reduce((sum, value) => sum + (value - mean) ** 2, 0) / samples.length;
  return {
    min: sorted[0],
    median: percentile(sorted, 0.5),
    mean,
    stddev: Math.sqrt(variance),
    p95: percentile(sorted, 0.95),
    max: sorted[sorted.length - 1],
  };
}

function percentile(sortedSamples, fraction) {
  const rank = Math.max(0, Math.ceil(fraction * sortedSamples.length) - 1);
  return sortedSamples[rank];
}

function utf8Bytes(value) {
  return Buffer.byteLength(value, 'utf8');
}

function characterCount(value) {
  return [...value].length;
}

function estimateTokens(value) {
  return Math.ceil(characterCount(value) / 4);
}

function roundMetrics(value) {
  if (Array.isArray(value)) {
    return value.map(roundMetrics);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, roundMetrics(item)]),
    );
  }
  return typeof value === 'number' && !Number.isInteger(value) ? Number(value.toFixed(3)) : value;
}

module.exports = {
  characterCount,
  estimateTokens,
  roundMetrics,
  summarize,
  utf8Bytes,
};
