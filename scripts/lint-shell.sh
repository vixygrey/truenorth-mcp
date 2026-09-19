#!/usr/bin/env bash
# Lint the repo's shell scripts with the pinned shellcheck version, so a local run matches
# the CI `shell` job (issue #227).
#
# The pinned version is read from .github/workflows/ci.yml (SHELLCHECK_VERSION), so the
# workflow is the single source of truth and a bump is one line there.
#
# It prefers a local shellcheck that already matches the pin. Otherwise it runs the pinned
# koalaman/shellcheck container through Docker. With neither a matching local shellcheck nor
# Docker, it explains what to install and exits non-zero.
#
# Usage: bash scripts/lint-shell.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

WORKFLOW=".github/workflows/ci.yml"

# Read the pin from the workflow. The value looks like `SHELLCHECK_VERSION: v0.11.0`.
PIN=$(grep -E '^\s*SHELLCHECK_VERSION:' "$WORKFLOW" | head -n1 | sed -E 's/.*:\s*//' | tr -d '"'"'"' ')
if [ -z "$PIN" ]; then
  echo "lint-shell: could not read SHELLCHECK_VERSION from $WORKFLOW." >&2
  exit 1
fi

# The tracked shell scripts under skills/ and scripts/. A git glob, not a shell glob.
mapfile -t FILES < <(git ls-files 'skills/**/*.sh' 'scripts/**/*.sh')
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "lint-shell: no shell scripts to lint."
  exit 0
fi

# The pin carries a leading `v`; the shellcheck --version output does not.
PIN_NUMBER="${PIN#v}"

run_local() {
  local have
  have=$(shellcheck --version 2>/dev/null | sed -nE 's/^version: (.*)$/\1/p')
  [ "$have" = "$PIN_NUMBER" ] || return 1
  echo "lint-shell: using local shellcheck $have (matches the pin)."
  shellcheck --severity=warning "${FILES[@]}"
}

run_docker() {
  command -v docker >/dev/null 2>&1 || return 1
  echo "lint-shell: using the koalaman/shellcheck:$PIN container."
  docker run --rm -v "$REPO_ROOT:/mnt" -w /mnt "koalaman/shellcheck:$PIN" \
    --severity=warning "${FILES[@]}"
}

if run_local; then
  exit 0
fi

if run_docker; then
  exit 0
fi

cat >&2 <<EOF
lint-shell: no shellcheck matching the pinned $PIN, and Docker is not available.
Install one of:
  - shellcheck $PIN on your PATH, or
  - Docker, so this script can run koalaman/shellcheck:$PIN.
EOF
exit 1
