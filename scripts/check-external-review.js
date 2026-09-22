'use strict';

const fs = require('node:fs');

function reviewSortKey(review) {
  return `${review.submitted_at ?? ''}\u0000${String(review.id ?? '')}`;
}

function latestReview(reviews) {
  return [...reviews]
    .sort((left, right) => reviewSortKey(left).localeCompare(reviewSortKey(right)))
    .at(-1);
}

function evaluateExternalReview({ author, headSha, maintainers, reviews }) {
  if (maintainers.includes(author)) {
    return { allowed: true, reason: 'The pull request author is a maintainer.' };
  }

  const currentApprovals = maintainers.flatMap((maintainer) => {
    const review = latestReview(
      reviews.filter((candidate) => candidate.user?.login === maintainer),
    );
    return review?.commit_id === headSha && review.state === 'APPROVED' ? [maintainer] : [];
  });

  if (currentApprovals.length > 0) {
    return {
      allowed: true,
      reason: `Current-head approval recorded from ${currentApprovals.join(', ')}.`,
    };
  }

  return {
    allowed: false,
    reason: 'An external pull request needs a current-head approval from an allowed maintainer.',
  };
}

function main() {
  const [reviewsPath] = process.argv.slice(2);
  if (!reviewsPath) {
    throw new Error('usage: check-external-review.js <reviews.json>');
  }

  const author = process.env.PR_AUTHOR;
  const headSha = process.env.PR_HEAD_SHA;
  const maintainers = (process.env.ALLOWED_MAINTAINERS ?? '')
    .split(',')
    .map((maintainer) => maintainer.trim())
    .filter(Boolean);

  if (!author || !headSha || maintainers.length === 0) {
    throw new Error('PR_AUTHOR, PR_HEAD_SHA, and ALLOWED_MAINTAINERS must be set.');
  }

  const reviews = JSON.parse(fs.readFileSync(reviewsPath, 'utf8'));
  const outcome = evaluateExternalReview({ author, headSha, maintainers, reviews });
  console.log(outcome.reason);
  if (!outcome.allowed) {
    process.exitCode = 1;
  }
}

if (require.main === module) {
  main();
}

module.exports = { evaluateExternalReview };
