#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
SCRIPT="$ROOT/skills/release-branch/scripts/check-route.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

pass=0
fail=0

report_pass() {
  printf 'PASS: %s\n' "$1"
  pass=$((pass + 1))
}

report_fail() {
  printf 'FAIL: %s\n' "$1" >&2
  fail=$((fail + 1))
}

make_git() {
  local bin_dir="$1"
  mkdir -p "$bin_dir"
  cat >"$bin_dir/git" <<'EOF'
#!/bin/bash
if [ "${1:-}" = "rev-parse" ]; then
  printf 'true\n'
fi
exit 0
EOF
  chmod +x "$bin_dir/git"
}

make_non_git() {
  local bin_dir="$1"
  mkdir -p "$bin_dir"
  cat >"$bin_dir/git" <<'EOF'
#!/bin/bash
exit 1
EOF
  chmod +x "$bin_dir/git"
}

make_gh() {
  local bin_dir="$1"
  cat >"$bin_dir/gh" <<'EOF'
#!/bin/bash
exit 0
EOF
  chmod +x "$bin_dir/gh"
}

assert_success() {
  local name="$1"
  shift
  if "$@" >"$TMP/stdout" 2>"$TMP/stderr"; then
    report_pass "$name"
  else
    cat "$TMP/stderr" >&2
    report_fail "$name"
  fi
}

assert_failure() {
  local name="$1"
  shift
  if "$@" >"$TMP/stdout" 2>"$TMP/stderr"; then
    report_fail "$name"
  else
    report_pass "$name"
  fi
}

solo_bin="$TMP/solo-bin"
make_git "$solo_bin"
assert_success "solo Git needs only git" \
  env PATH="$solo_bin" /bin/bash "$SCRIPT" --mode solo-git --action inspect
if grep -q '"route":"solo-git"' "$TMP/stdout"; then
  report_pass "solo Git route is explicit"
else
  report_fail "solo Git route is explicit"
fi

team_bin="$TMP/team-bin"
make_git "$team_bin"
assert_failure "team PR rejects a missing GitHub CLI" \
  env PATH="$team_bin" /bin/bash "$SCRIPT" --mode team-pr --action inspect
make_gh "$team_bin"
assert_success "team PR accepts git and GitHub CLI" \
  env PATH="$team_bin" /bin/bash "$SCRIPT" --mode team-pr --action inspect

assert_failure "missing workflow mode fails closed" \
  env PATH="$solo_bin" /bin/bash "$SCRIPT" --action inspect
assert_failure "unknown workflow mode fails closed" \
  env PATH="$solo_bin" /bin/bash "$SCRIPT" --mode automatic --action inspect
non_git_bin="$TMP/non-git-bin"
make_non_git "$non_git_bin"
assert_failure "unsupported Jujutsu workflow fails closed" \
  env PATH="$non_git_bin" /bin/bash "$SCRIPT" --mode solo-git --action inspect

for action in merge tag publish cleanup; do
  assert_failure "$action requires explicit approval" \
    env PATH="$solo_bin" /bin/bash "$SCRIPT" --mode solo-git --action "$action"
  assert_success "$action accepts explicit approval" \
    env PATH="$solo_bin" /bin/bash "$SCRIPT" --mode solo-git --action "$action" --approved
done

printf '%s passed, %s failed\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
