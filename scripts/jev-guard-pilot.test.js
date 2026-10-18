'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { classifyFindings } = require('./jev-guard-pilot.js');

const rigor = {
  hallucinated_import: 0.7,
  violates_conventions: 0.69,
  contains_secrets: 0.25,
  complexity: 62.5,
};
const drift = { noul: 0.5, forced_by_path_match: false, out_of_scope: true };

function guardBody(findings = {}) {
  return {
    decision: 'annotate',
    notes: ['Advisory'],
    findings: {
      unavailable: null,
      unavailable_context: [],
      rigor,
      drift,
      ...findings,
    },
  };
}

test('a finding mismatch retains bounded rigor scores without retaining provider fields', () => {
  const result = classifyFindings(
    guardBody({ rigor: { ...rigor, provider_private: 'do not report' } }),
    { hallucinated_import: false },
  );
  assert.equal(result.matches, false);
  assert.equal(result.unavailable, null);
  assert.deepEqual(result.rigor_scores, rigor);
  assert.equal(result.observed.hallucinated_import, true);
});

test('valid rigor survives unavailable drift but never yields a negative finding', () => {
  for (const findings of [
    { drift: null },
    { unavailable: 'model unavailable' },
    { drift: { ...drift, noul: null } },
  ]) {
    const result = classifyFindings(guardBody(findings), { hallucinated_import: false });
    assert.deepEqual(result.rigor_scores, rigor);
    assert.equal(result.observed, null);
    assert.equal(result.matches, false);
    assert.notEqual(result.unavailable, null);
  }
});

test('missing or invalid rigor cannot become valid scores or negative findings', () => {
  for (const invalid of [
    null,
    { ...rigor, hallucinated_import: NaN },
    { ...rigor, violates_conventions: Infinity },
    { ...rigor, contains_secrets: -0.01 },
    { ...rigor, complexity: 100.01 },
    { ...rigor, complexity: '62.5' },
    { ...rigor, hallucinated_import: undefined },
  ]) {
    const result = classifyFindings(guardBody({ rigor: invalid }), {
      hallucinated_import: false,
    });
    assert.equal(result.rigor_scores, null);
    assert.equal(result.observed, null);
    assert.equal(result.matches, false);
    assert.notEqual(result.unavailable, null);
  }
});

test('threshold boundaries classify rigor and drift without changing policy', () => {
  const exact = classifyFindings(guardBody(), {
    hallucinated_import: true,
    violates_conventions: false,
    contains_secrets: true,
    complexity_high: true,
    drift_out_of_scope: true,
  });
  assert.equal(exact.matches, true);
  assert.deepEqual(exact.rigor_scores, rigor);
  const below = classifyFindings(
    guardBody({
      rigor: {
        hallucinated_import: 0.699,
        violates_conventions: 0.699,
        contains_secrets: 0.249,
        complexity: 62.499,
      },
      drift: { noul: 0.499, forced_by_path_match: false, out_of_scope: false },
    }),
    {
      hallucinated_import: false,
      violates_conventions: false,
      contains_secrets: false,
      complexity_high: false,
      drift_out_of_scope: false,
    },
  );
  assert.equal(below.matches, true);
  assert.deepEqual(below.observed, {
    hallucinated_import: false,
    violates_conventions: false,
    contains_secrets: false,
    complexity_high: false,
    drift_out_of_scope: false,
  });
});
