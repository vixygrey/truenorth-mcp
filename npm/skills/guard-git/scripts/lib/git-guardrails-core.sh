# shellcheck shell=bash
# Shared policy for git hook guardrails.
# Source from block-dangerous-git.sh only. This file is sourced, not executed, so it
# carries a `shell` directive rather than a shebang.
#
# This library holds three policy checks, each a pure-ish function that reads its input
# from arguments and prints a machine-readable result. The pre-command hook wires them to
# the harness contract (stderr + exit 2, or a JSON decision).
#
# 1. Dangerous-pattern match: string match only, always on.
# 2. Branch protection: opt-in, reads the current branch for a commit or a push.
# 3. Conventional Commits: opt-in, parses the -m subject from the command string.
#
# Design and decision: issue #210.

# ---------------------------------------------------------------------------
# 1. Dangerous-pattern match (unchanged behavior)
# ---------------------------------------------------------------------------

GIT_GUARDRAILS_PATTERNS=(
  "git reset --hard"
  "git clean -fd"
  "git clean -f"
  "git branch -D"
  "git checkout \\."
  "git restore \\."
  "push --force"
  "reset --hard"
)

# Print the first matching dangerous pattern to stdout; return 0 if dangerous, 1 if safe.
# Usage: if pattern=$(git_guardrails_first_match "$command"); then ... ; fi
git_guardrails_first_match() {
  local cmd="$1"
  local p
  for p in "${GIT_GUARDRAILS_PATTERNS[@]}"; do
    if echo "$cmd" | grep -qE -- "$p"; then
      printf '%s' "$p"
      return 0
    fi
  done
  return 1
}

# ---------------------------------------------------------------------------
# 2. Branch protection (opt-in: GIT_GUARDRAILS_PROTECT_BRANCH=1)
# ---------------------------------------------------------------------------

# The protected branch names. A commit on one of these, or a push targeting one, is
# blocked unless the land hatch is set.
GIT_GUARDRAILS_PROTECTED_BRANCHES=(main master)

# Report whether a name is a protected branch. Return 0 when protected, 1 otherwise.
git_guardrails_is_protected_branch() {
  local name="$1"
  local b
  for b in "${GIT_GUARDRAILS_PROTECTED_BRANCHES[@]}"; do
    if [ "$name" = "$b" ]; then
      return 0
    fi
  done
  return 1
}

# Classify a command as a git commit, a git push, or neither. Print `commit`, `push`, or
# nothing. The classification is a string match, so it needs no repository access.
git_guardrails_git_action() {
  local cmd="$1"
  # A commit: `git commit ...`. A push: `git push ...`. Match the git subcommand token,
  # not a substring inside another word.
  if echo "$cmd" | grep -qE '(^|[[:space:]])git[[:space:]]+commit([[:space:]]|$)'; then
    printf 'commit'
    return 0
  fi
  if echo "$cmd" | grep -qE '(^|[[:space:]])git[[:space:]]+push([[:space:]]|$)'; then
    printf 'push'
    return 0
  fi
  return 1
}

# Read the current branch via git. Print the branch name on success, nothing on failure.
# Return 0 on success, 1 when git cannot report a branch (outside a repo, detached HEAD).
# This is the only function here that reads repository state.
git_guardrails_current_branch() {
  local branch
  # `git rev-parse --abbrev-ref HEAD` prints `HEAD` for a detached head, which is not a
  # protected name, so a detached head falls through to allow (fail open).
  branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null) || return 1
  [ -n "$branch" ] || return 1
  printf '%s' "$branch"
}

# Extract the explicit push target branch from a command, when present.
# Handles `git push <remote> <branch>` and `git push <remote> HEAD:<branch>`. Prints the
# branch name, or nothing when the push has no explicit branch argument.
git_guardrails_push_target_branch() {
  local cmd="$1"
  # Drop everything up to and including the `push` token, then read the arguments.
  local args
  args=$(echo "$cmd" | sed -E 's/.*[[:space:]]push[[:space:]]+//')
  # Split into whitespace tokens, skip option flags, take remote then branch.
  local tokens=()
  local t
  # shellcheck disable=SC2206  # deliberate word split on the sanitized argument string.
  tokens=($args)
  local positional=()
  for t in "${tokens[@]}"; do
    case "$t" in
      -*) : ;; # skip a flag such as -u or --set-upstream.
      *) positional+=("$t") ;;
    esac
  done
  # positional[0] is the remote, positional[1] is the refspec.
  local refspec="${positional[1]:-}"
  [ -n "$refspec" ] || return 1
  # A refspec `HEAD:main` or `local:main` targets the part after the colon.
  case "$refspec" in
    *:*) printf '%s' "${refspec##*:}" ;;
    *) printf '%s' "$refspec" ;;
  esac
}

# Decide whether branch protection blocks a command.
# Prints a human-readable reason and returns 0 when the command must be blocked.
# Returns 1 (allow) when protection is off, the land hatch is set, the action is neither a
# commit nor a push, or the relevant branch is not protected or cannot be read (fail open).
git_guardrails_branch_block_reason() {
  local cmd="$1"
  # Opt-in only.
  [ "${GIT_GUARDRAILS_PROTECT_BRANCH:-0}" = "1" ] || return 1
  # The deliberate land hatch bypasses branch protection for the intended land flow.
  [ "${GIT_GUARDRAILS_LAND:-0}" = "1" ] && return 1

  local action
  action=$(git_guardrails_git_action "$cmd") || return 1

  local target=""
  case "$action" in
    commit)
      # A commit lands on the current branch. Read it; fail open when unavailable.
      target=$(git_guardrails_current_branch) || return 1
      ;;
    push)
      # Prefer the explicit push target; fall back to the current branch.
      target=$(git_guardrails_push_target_branch "$cmd") || target=$(git_guardrails_current_branch) || return 1
      ;;
  esac

  [ -n "$target" ] || return 1
  if git_guardrails_is_protected_branch "$target"; then
    printf "a direct %s to the protected branch '%s' is blocked. Open a branch and a pull request, or set GIT_GUARDRAILS_LAND=1 for the deliberate land flow." "$action" "$target"
    return 0
  fi
  return 1
}

# ---------------------------------------------------------------------------
# 3. Conventional Commits subject validation (opt-in: GIT_GUARDRAILS_CONVENTIONAL=1)
# ---------------------------------------------------------------------------

# The approved Conventional Commits types.
GIT_GUARDRAILS_CC_TYPES="feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert"

# Extract the commit subject from a `git commit -m <subject>` command.
# Prints the subject and returns 0 when a single-line -m/--message value is found.
# Returns 1 when the command uses -F/--file, a heredoc, or no extractable -m value, so the
# caller skips validation (fail open); the commit-msg git hook validates those paths.
git_guardrails_extract_commit_subject() {
  local cmd="$1"
  # A file-sourced or heredoc message is not on the command line. Skip it.
  case "$cmd" in
    *"-F"*|*"--file"*|*"<<"*) return 1 ;;
  esac
  # Match -m or --message followed by a single- or double-quoted value, and print the
  # quoted content. This deliberately handles only a quoted single-line subject; anything
  # else returns 1 and is skipped.
  local subject
  subject=$(printf '%s' "$cmd" | sed -nE "s/.*(-m|--message)[[:space:]]+\"([^\"]*)\".*/\2/p")
  if [ -z "$subject" ]; then
    subject=$(printf '%s' "$cmd" | sed -nE "s/.*(-m|--message)[[:space:]]+'([^']*)'.*/\2/p")
  fi
  [ -n "$subject" ] || return 1
  printf '%s' "$subject"
}

# Report whether a subject matches Conventional Commits. Return 0 when valid, 1 otherwise.
# Format: type(optional-scope)!: description, with an approved type.
git_guardrails_is_conventional_subject() {
  local subject="$1"
  printf '%s' "$subject" | grep -qE "^(${GIT_GUARDRAILS_CC_TYPES})(\([a-z0-9._-]+\))?!?: .+"
}

# Decide whether Conventional Commits enforcement blocks a command.
# Prints a reason and returns 0 when the command must be blocked. Returns 1 (allow) when
# enforcement is off, the command is not a commit, the subject cannot be extracted (fail
# open), or the subject is already conventional.
git_guardrails_conventional_block_reason() {
  local cmd="$1"
  [ "${GIT_GUARDRAILS_CONVENTIONAL:-0}" = "1" ] || return 1

  local action
  action=$(git_guardrails_git_action "$cmd") || return 1
  [ "$action" = "commit" ] || return 1

  local subject
  subject=$(git_guardrails_extract_commit_subject "$cmd") || return 1

  if git_guardrails_is_conventional_subject "$subject"; then
    return 1
  fi
  printf "the commit subject '%s' is not a Conventional Commit. Use type(scope): description with a type of %s." "$subject" "$GIT_GUARDRAILS_CC_TYPES"
  return 0
}
