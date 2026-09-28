#!/usr/bin/env bash
# Run every scripted skill's declared behavioral verification.
#
# Usage: bash scripts/verify-skills.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if ! command -v node >/dev/null 2>&1; then
  echo "verify-skills: Node.js 18 or newer is required." >&2
  exit 1
fi

node scripts/verify-scripted-skills.js
