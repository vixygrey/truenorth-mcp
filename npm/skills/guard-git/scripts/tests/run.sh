#!/usr/bin/env bash
# The guard-git test harness. Runs every policy check against the shipped scripts and
# reports pass/fail. Dependency-light: git, jq, grep, bash. Issue #210.
#
# Usage: bash skills/guard-git/scripts/tests/run.sh
# Exit 0 when every test passes, 1 otherwise.

set -uo pipefail

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="$(cd "$TESTS_DIR/.." && pwd)"
HOOK="$SCRIPTS_DIR/block-dangerous-git.sh"
SECRET_HOOK="$SCRIPTS_DIR/pre-commit-secret-scan.sh"

PASS=0
FAIL=0

ok() {
  PASS=$((PASS + 1))
  printf 'ok   %s\n' "$1"
}

no() {
  FAIL=$((FAIL + 1))
  printf 'FAIL %s\n' "$1"
  [ -n "${2:-}" ] && printf '     %s\n' "$2"
}

# Assert an exit code recorded in /tmp/bc.$$ equals the expected value.
assert_recorded_code() {
  local expected="$1" name="$2"
  local actual
  actual=$(cat /tmp/bc.$$)
  if [ "$actual" = "$expected" ]; then
    ok "$name"
  else
    no "$name" "code=$actual"
  fi
}

# Run the pre-command hook with a command payload. Sets REPLY_OUT, REPLY_ERR, REPLY_CODE.
run_hook() {
  local command="$1"
  local payload
  payload=$(jq -nc --arg c "$command" '{tool_input: {command: $c}}')
  REPLY_OUT=$(printf '%s' "$payload" | "$HOOK" 2>/tmp/guard_err.$$)
  REPLY_CODE=$?
  REPLY_ERR=$(cat /tmp/guard_err.$$)
  rm -f /tmp/guard_err.$$
}

# Assert a claude-mode block: exit 2 and a BLOCKED message on stderr.
assert_claude_block() {
  local name="$1"
  if [ "$REPLY_CODE" = "2" ] && printf '%s' "$REPLY_ERR" | grep -q "BLOCKED"; then
    ok "$name"
  else
    no "$name" "code=$REPLY_CODE err=$REPLY_ERR"
  fi
}

# Assert an allow: exit 0 and no BLOCKED message.
assert_allow() {
  local name="$1"
  if [ "$REPLY_CODE" = "0" ] && ! printf '%s' "$REPLY_ERR" | grep -q "BLOCKED"; then
    ok "$name"
  else
    no "$name" "code=$REPLY_CODE err=$REPLY_ERR out=$REPLY_OUT"
  fi
}

# Assert a gemini-mode deny: exit 0 and a deny decision on stdout.
assert_gemini_deny() {
  local name="$1"
  local decision
  decision=$(printf '%s' "$REPLY_OUT" | jq -r '.decision // empty' 2>/dev/null)
  if [ "$REPLY_CODE" = "0" ] && [ "$decision" = "deny" ]; then
    ok "$name"
  else
    no "$name" "code=$REPLY_CODE out=$REPLY_OUT"
  fi
}

# ---------------------------------------------------------------------------
# 1. Dangerous-pattern match
# ---------------------------------------------------------------------------
run_hook "git reset --hard HEAD~1"; assert_claude_block "pattern: reset --hard blocked"
run_hook "git push --force origin main"; assert_claude_block "pattern: push --force blocked"
run_hook "git clean -fd"; assert_claude_block "pattern: clean -fd blocked"
run_hook "git status"; assert_allow "pattern: git status allowed"

# Gemini mode: deny on stdout, allow on stdout.
REPLY_OUT=$(printf '{"tool_input":{"command":"git clean -fd"}}' | GIT_GUARDRAILS_MODE=gemini "$HOOK"); REPLY_CODE=$?
assert_gemini_deny "pattern: gemini deny on dangerous"
REPLY_OUT=$(printf '{"tool_input":{"command":"git status"}}' | GIT_GUARDRAILS_MODE=gemini "$HOOK"); REPLY_CODE=$?
if [ "$REPLY_CODE" = "0" ] && [ "$(printf '%s' "$REPLY_OUT" | jq -r '.decision')" = "allow" ]; then
  ok "pattern: gemini allow on safe"
else
  no "pattern: gemini allow on safe" "out=$REPLY_OUT"
fi

# ---------------------------------------------------------------------------
# 2. Branch protection (opt-in, reads repo state)
# ---------------------------------------------------------------------------
# Build a hook payload for a command with jq, so embedded quotes stay valid JSON.
payload_for() {
  jq -nc --arg c "$1" '{tool_input: {command: $c}}'
}

# A hermetic temp repo on `main`.
REPO=$(mktemp -d)
(
  cd "$REPO" || exit 1
  git init -q -b main
  git config user.email t@t.invalid
  git config user.name t
  echo x > f && git add f && git commit -qm "chore: seed"
) >/dev/null 2>&1

COMMIT_CMD='git commit -m "x"'

# From inside the protected branch, a commit is blocked when protection is on.
(
  cd "$REPO" || exit 1
  export GIT_GUARDRAILS_PROTECT_BRANCH=1
  payload_for "$COMMIT_CMD" | "$HOOK" 2>/tmp/be.$$
  echo $? > /tmp/bc.$$
)
if [ "$(cat /tmp/bc.$$)" = "2" ] && grep -q "protected branch 'main'" /tmp/be.$$; then
  ok "branch: commit on main blocked"
else
  no "branch: commit on main blocked" "code=$(cat /tmp/bc.$$) err=$(cat /tmp/be.$$)"
fi

# The land hatch bypasses protection.
(
  cd "$REPO" || exit 1
  export GIT_GUARDRAILS_PROTECT_BRANCH=1 GIT_GUARDRAILS_LAND=1
  payload_for "$COMMIT_CMD" | "$HOOK" 2>/dev/null
  echo $? > /tmp/bc.$$
)
assert_recorded_code 0 "branch: land hatch allows commit on main"

# A push targeting main is blocked.
(
  cd "$REPO" || exit 1
  export GIT_GUARDRAILS_PROTECT_BRANCH=1
  printf '{"tool_input":{"command":"git push origin main"}}' | "$HOOK" 2>/tmp/be.$$
  echo $? > /tmp/bc.$$
)
if [ "$(cat /tmp/bc.$$)" = "2" ] && grep -q "protected branch 'main'" /tmp/be.$$; then
  ok "branch: push to main blocked"
else
  no "branch: push to main blocked" "code=$(cat /tmp/bc.$$) err=$(cat /tmp/be.$$)"
fi

# A push targeting a feature branch is allowed.
(
  cd "$REPO" || exit 1
  export GIT_GUARDRAILS_PROTECT_BRANCH=1
  printf '{"tool_input":{"command":"git push origin feat/x"}}' | "$HOOK" 2>/dev/null
  echo $? > /tmp/bc.$$
)
assert_recorded_code 0 "branch: push to feature branch allowed"

# Protection off by default: a commit on main is allowed.
(
  cd "$REPO" || exit 1
  payload_for "$COMMIT_CMD" | "$HOOK" 2>/dev/null
  echo $? > /tmp/bc.$$
)
assert_recorded_code 0 "branch: protection off allows commit on main"

# Outside a git repo: fail open even with protection on.
(
  cd "$(mktemp -d)" || exit 1
  export GIT_GUARDRAILS_PROTECT_BRANCH=1
  payload_for "$COMMIT_CMD" | "$HOOK" 2>/dev/null
  echo $? > /tmp/bc.$$
)
assert_recorded_code 0 "branch: outside a repo fails open"

rm -f /tmp/bc.$$ /tmp/be.$$
rm -rf "$REPO"

# ---------------------------------------------------------------------------
# 3. Conventional Commits (opt-in, string parse)
# ---------------------------------------------------------------------------
export GIT_GUARDRAILS_CONVENTIONAL=1

run_hook 'git commit -m "feat: add a thing"'; assert_allow "cc: valid feat subject allowed"
run_hook 'git commit -m "fix(scope): correct a bug"'; assert_allow "cc: valid scoped fix allowed"
run_hook 'git commit -m "feat!: breaking change"'; assert_allow "cc: valid breaking-bang allowed"
run_hook 'git commit -m "update stuff"'; assert_claude_block "cc: non-conventional subject blocked"
run_hook 'git commit -m "wip"'; assert_claude_block "cc: bare wip blocked"
# A -F file message is skipped (fail open).
run_hook 'git commit -F /tmp/msg.txt'; assert_allow "cc: -F message skipped"
# A heredoc message is skipped (fail open).
run_hook 'git commit -F - <<EOF'; assert_allow "cc: heredoc message skipped"
# A non-commit git command is not validated.
run_hook 'git status'; assert_allow "cc: non-commit command not validated"

unset GIT_GUARDRAILS_CONVENTIONAL

# ---------------------------------------------------------------------------
# 4. Secret scanning (pre-commit hook over git diff --cached)
# ---------------------------------------------------------------------------
SREPO=$(mktemp -d)
(
  cd "$SREPO" || exit 1
  git init -q -b main
  git config user.email t@t.invalid
  git config user.name t
) >/dev/null 2>&1

# A staged secret is blocked.
(
  cd "$SREPO" || exit 1
  printf 'token = "ghp_%s"\n' "0123456789abcdefghijABCDEF" > cfg.txt
  git add cfg.txt
  "$SECRET_HOOK" 2>/tmp/se.$$
  echo $? > /tmp/sc.$$
)
if [ "$(cat /tmp/sc.$$)" = "1" ] && grep -q "GitHub token" /tmp/se.$$; then
  ok "secret: staged GitHub token blocked"
else
  no "secret: staged GitHub token blocked" "code=$(cat /tmp/sc.$$) err=$(cat /tmp/se.$$)"
fi

# A clean staged diff is allowed.
(
  cd "$SREPO" || exit 1
  git rm -q --cached cfg.txt >/dev/null 2>&1
  rm -f cfg.txt
  echo "just some text" > ok.txt
  git add ok.txt
  "$SECRET_HOOK" 2>/dev/null
  echo $? > /tmp/sc.$$
)
if [ "$(cat /tmp/sc.$$)" = "0" ]; then ok "secret: clean staged diff allowed"; else no "secret: clean staged diff allowed"; fi

# A private-key block is blocked.
(
  cd "$SREPO" || exit 1
  printf -- '-----BEGIN RSA PRIVATE KEY-----\n' > key.pem
  git add key.pem
  "$SECRET_HOOK" 2>/tmp/se.$$
  echo $? > /tmp/sc.$$
)
if [ "$(cat /tmp/sc.$$)" = "1" ] && grep -q "private key" /tmp/se.$$; then
  ok "secret: staged private key blocked"
else
  no "secret: staged private key blocked" "code=$(cat /tmp/sc.$$) err=$(cat /tmp/se.$$)"
fi

rm -f /tmp/sc.$$ /tmp/se.$$
rm -rf "$SREPO"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" = "0" ]
