'use strict';

const assert = require('node:assert');
const { test } = require('node:test');

const { evaluateExternalReview } = require('../../scripts/check-external-review.js');

const MAINTAINERS = ['vixygrey'];
const HEAD = 'current-head';

function review({
  id,
  author = 'vixygrey',
  commit = HEAD,
  state,
  submittedAt = '2026-09-22T00:00:00Z',
}) {
  return {
    id,
    user: { login: author },
    commit_id: commit,
    state,
    submitted_at: submittedAt,
  };
}

function evaluate(overrides = {}) {
  return evaluateExternalReview({
    author: 'external-contributor',
    headSha: HEAD,
    maintainers: MAINTAINERS,
    reviews: [],
    ...overrides,
  });
}

test('allows a maintainer-authored pull request without review', () => {
  assert.deepEqual(evaluate({ author: 'vixygrey' }), {
    allowed: true,
    reason: 'The pull request author is a maintainer.',
  });
});

test('blocks an external pull request without a review', () => {
  assert.equal(evaluate().allowed, false);
});

test('blocks an approval for an earlier pull request head', () => {
  assert.equal(
    evaluate({ reviews: [review({ id: 1, commit: 'earlier-head', state: 'APPROVED' })] }).allowed,
    false,
  );
});

test('allows a current-head maintainer approval', () => {
  assert.equal(evaluate({ reviews: [review({ id: 1, state: 'APPROVED' })] }).allowed, true);
});

test('blocks a later maintainer change request', () => {
  assert.equal(
    evaluate({
      reviews: [
        review({ id: 1, state: 'APPROVED' }),
        review({
          id: 2,
          state: 'CHANGES_REQUESTED',
          submittedAt: '2026-09-22T00:01:00Z',
        }),
      ],
    }).allowed,
    false,
  );
});

test('allows a later maintainer approval after a change request', () => {
  assert.equal(
    evaluate({
      reviews: [
        review({ id: 1, state: 'CHANGES_REQUESTED' }),
        review({ id: 2, state: 'APPROVED', submittedAt: '2026-09-22T00:01:00Z' }),
      ],
    }).allowed,
    true,
  );
});

test('blocks an approval from an unauthorized reviewer', () => {
  assert.equal(
    evaluate({ reviews: [review({ id: 1, author: 'untrusted-reviewer', state: 'APPROVED' })] })
      .allowed,
    false,
  );
});
