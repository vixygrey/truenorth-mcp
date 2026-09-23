#!/usr/bin/env bash
# Blocks dangerous git commands for Claude Code, Cursor, and Gemini CLI hooks.
# Requires jq on PATH.
#
# GIT_GUARDRAILS_MODE: claude (default) | cursor | gemini
#   claude/cursor: stderr message, exit 2 on block, exit 0 on allow
#   gemini: JSON with decision on stdout, exit 0 always (allow or deny)
#
# Opt-in policies (off by default; see REFERENCE.md and issue #210):
#   GIT_GUARDRAILS_PROTECT_BRANCH=1  block a direct commit/push to main or master
#   GIT_GUARDRAILS_LAND=1            bypass branch protection for the deliberate land flow
#   GIT_GUARDRAILS_CONVENTIONAL=1    require a Conventional Commits subject on git commit -m

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/git-guardrails-core.sh
. "$SCRIPT_DIR/lib/git-guardrails-core.sh"

INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.command // .tool_input.command // empty')
MODE="${GIT_GUARDRAILS_MODE:-claude}"

# Emit a block in the mode's contract, then exit.
emit_block() {
  local reason="$1"
  case "$MODE" in
    gemini)
      jq -nc --arg reason "$reason" '{decision: "deny", reason: $reason}'
      exit 0
      ;;
    claude | cursor | *)
      echo "$reason" >&2
      exit 2
      ;;
  esac
}

# Emit an allow in the mode's contract, then exit.
emit_allow() {
  if [ "$MODE" = "gemini" ]; then
    echo '{"decision":"allow"}'
  fi
  exit 0
}

if [ -z "$COMMAND" ]; then
  emit_allow
fi

# 1. Dangerous-pattern match (always on).
if PATTERN=$(git_guardrails_first_match "$COMMAND"); then
  emit_block "BLOCKED: '$COMMAND' matches dangerous pattern '$PATTERN'. The user has prevented you from doing this."
fi

# 2. Branch protection (opt-in).
if REASON=$(git_guardrails_branch_block_reason "$COMMAND"); then
  emit_block "BLOCKED: $REASON"
fi

# 3. Conventional Commits (opt-in).
if REASON=$(git_guardrails_conventional_block_reason "$COMMAND"); then
  emit_block "BLOCKED: $REASON"
fi

emit_allow
