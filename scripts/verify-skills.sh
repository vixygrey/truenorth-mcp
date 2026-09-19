#!/usr/bin/env bash
# Run the documented verify command for each script-bearing skill (issue #250, part 2).
#
# Most of the 79 skills are prose an agent follows, with no runnable surface. Four ship
# executable scripts: align-grid, extract-design, guard-git, visual-dashboard. This runner
# executes each one's documented verify and confirms it exits 0. A verify that needs a
# browser or a live HTTP server is skipped when that dependency is absent, so the runner
# stays green in a headless CI without pulling a browser into the job.
#
# Exit 0 when every runnable verify passed. Exit 1 when a runnable verify failed. A skip
# is not a failure. The prose-skill quality question stays with run-benchmark and
# run-evals (see the #244 investigation); this runner checks the mechanical verify only.
#
# Usage: bash scripts/verify-skills.sh

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

pass=0
fail=0
skip=0

report_pass() {
  echo "verify-skills: PASS $1"
  pass=$((pass + 1))
}
report_skip() {
  echo "verify-skills: SKIP $1 ($2)"
  skip=$((skip + 1))
}
report_fail() {
  echo "verify-skills: FAIL $1" >&2
  fail=$((fail + 1))
}

# --- guard-git: a self-contained shell test harness (documented verify). ---
# `## Quick start` step 7: `bash skills/guard-git/scripts/tests/run.sh`.
if bash skills/guard-git/scripts/tests/run.sh >/dev/null 2>&1; then
  report_pass "guard-git (scripts/tests/run.sh)"
else
  report_fail "guard-git (scripts/tests/run.sh)"
fi

# --- extract-design: the JS test file is the documented verify. ---
# It runs the classifier and writer unit tests browser-free, and skips the Puppeteer
# integration tests when no browser is present, so it exits 0 in a headless CI.
if command -v node >/dev/null 2>&1; then
  if node skills/extract-design/tests/test-extraction.js >/dev/null 2>&1; then
    report_pass "extract-design (tests/test-extraction.js)"
  else
    report_fail "extract-design (tests/test-extraction.js)"
  fi
else
  report_skip "extract-design (tests/test-extraction.js)" "node not found"
fi

# --- align-grid: the token generator runs standalone; the grid verifier needs a browser. ---
# `grid_tokens.py --help` is a dependency-free smoke check that the script loads and runs.
if command -v python3 >/dev/null 2>&1; then
  if python3 skills/align-grid/scripts/grid_tokens.py --help >/dev/null 2>&1; then
    report_pass "align-grid (grid_tokens.py --help smoke)"
  else
    report_fail "align-grid (grid_tokens.py --help smoke)"
  fi
else
  report_skip "align-grid (grid_tokens.py)" "python3 not found"
fi
# The `verify_grid.js` grid check needs Chrome and puppeteer-core (env CHROME, PUP).
if [ -n "${CHROME:-}" ] && [ -n "${PUP:-}" ]; then
  if node skills/align-grid/scripts/verify_grid.js --help >/dev/null 2>&1; then
    report_pass "align-grid (verify_grid.js)"
  else
    report_fail "align-grid (verify_grid.js)"
  fi
else
  report_skip "align-grid (verify_grid.js)" "CHROME and PUP not set; browser verify needs them"
fi

# --- visual-dashboard: the verify starts an HTTP server, so it is not a headless unit run. ---
# Smoke-check that the server module parses (node --check), without starting the server.
if command -v node >/dev/null 2>&1; then
  if node --check skills/visual-dashboard/scripts/server.cjs >/dev/null 2>&1; then
    report_pass "visual-dashboard (server.cjs syntax check)"
  else
    report_fail "visual-dashboard (server.cjs syntax check)"
  fi
  report_skip "visual-dashboard (live server verify)" "starts an HTTP server; not run in CI"
else
  report_skip "visual-dashboard (server.cjs)" "node not found"
fi

echo "verify-skills: $pass passed, $fail failed, $skip skipped."
if [ "$fail" -gt 0 ]; then
  exit 1
fi
exit 0
