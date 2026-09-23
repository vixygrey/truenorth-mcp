#!/usr/bin/env bash
# A git pre-commit hook that blocks a commit when the staged diff contains a common secret.
#
# The harness pre-command hook (block-dangerous-git.sh) sees the command string only, not
# the staged diff, so secret scanning lives here where `git diff --cached` is available
# (issue #210). Install it as `.git/hooks/pre-commit` or reference it from core.hooksPath.
#
# A match blocks the commit with exit 1 and a message that names the pattern and the file.
# Deeper supply-chain review stays with the audit-code skill.
#
# Bypass for a false positive with `git commit --no-verify`, and prefer redacting the value.

set -euo pipefail

# The secret patterns. Each entry is a label and an extended-regex. Keep it dependency-light
# (git and grep only) and aligned with the audit-code checklist and REFERENCE.md.
SECRET_LABELS=(
  "OpenAI API key"
  "GitHub token"
  "AWS access key id"
  "Slack bot token"
  "private key block"
)
SECRET_PATTERNS=(
  "sk-[A-Za-z0-9]{16,}"
  "gh[pous]_[A-Za-z0-9]{20,}"
  "AKIA[0-9A-Z]{16}"
  "xoxb-[A-Za-z0-9-]{10,}"
  "-----BEGIN [A-Z ]*PRIVATE KEY-----"
)

# Read the staged diff. Added lines only, so an unchanged pre-existing match does not block
# an unrelated commit. `--no-color` keeps the output plain for grep.
staged_added_lines() {
  # Limit to added lines (leading +) in the staged diff, dropping the +++ file headers.
  git diff --cached --no-color -U0 2>/dev/null | grep -E '^\+' | grep -Ev '^\+\+\+ '
}

main() {
  # Fail open outside a repo or when there is nothing staged: nothing to scan.
  local diff
  diff=$(staged_added_lines) || exit 0
  [ -n "$diff" ] || exit 0

  local i hit=0
  for i in "${!SECRET_PATTERNS[@]}"; do
    local pattern="${SECRET_PATTERNS[$i]}"
    local label="${SECRET_LABELS[$i]}"
    # `--` ends option parsing so a pattern that starts with `-` (a private-key header) is
    # read as a pattern, not a flag.
    if printf '%s\n' "$diff" | grep -qE -- "$pattern"; then
      echo "BLOCKED: the staged diff contains a $label pattern. Remove the secret before you commit." >&2
      hit=1
    fi
  done

  if [ "$hit" = "1" ]; then
    echo "If this is a false positive, redact the value or commit with --no-verify." >&2
    exit 1
  fi
  exit 0
}

main "$@"
