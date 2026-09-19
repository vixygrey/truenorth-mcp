#!/usr/bin/env bash
# Lint skill artifact-write targets against the convention in ADR-0014.
#
# The convention has two rules, one per artifact class:
#   - A machine artifact is written under `.agent/`.
#   - A transient render artifact is written to an out-of-repo dir (for example under
#     `/tmp`), or to a gitignored in-repo session dir.
# A skill never writes into `specs/`, which is human-authored and read-only to the
# runtime and its skills (ADR-0008).
#
# This lint checks the literal write targets in tracked skill scripts. It reports a
# write whose literal path names a repo-relative location outside `.agent/` and outside
# an allowed transient location. It does not resolve variables, so a write to a variable
# path (for example `$SESSION_DIR`) is checked at the constant that defines the literal,
# not at the call site.
#
# Usage: bash scripts/lint-skill-artifacts.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The tracked skill scripts. A git glob, not a shell glob, so detection matches CI.
mapfile -t FILES < <(git ls-files 'skills/**/*.js' 'skills/**/*.cjs' 'skills/**/*.mjs' 'skills/**/*.sh')
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "lint-skill-artifacts: no skill scripts to lint."
  exit 0
fi

# A write call names one of these forms. The lint extracts the first string literal
# argument on the line, then classifies that literal.
#   JS/CJS/MJS: writeFileSync, appendFileSync, createWriteStream, mkdirSync, cpSync,
#               renameSync, and the promise forms (writeFile, appendFile, mkdir, ...).
#   Shell:      a redirect `> path` or `>> path`, `tee path`, `mkdir -p path`.
# A path constant assignment (for example `const OUTPUT_PATH = '.agent/...'`) is also a
# write target, because the write call resolves it, so the lint checks those too.
WRITE_CALL_RE='(writeFileSync|appendFileSync|createWriteStream|mkdirSync|cpSync|copyFileSync|renameSync|writeFile|appendFile|mkdir|cp|rename)\s*\('
PATH_CONST_RE='(OUTPUT_PATH|STATE_PATH|SESSION_DIR|STATE_DIR|CONTENT_DIR|_PATH|_DIR)\s*='
SHELL_WRITE_RE='(^|[^0-9<>])>>?[[:space:]]*["'"'"']?[./]|(^|[[:space:]])tee[[:space:]]|(^|[[:space:]])mkdir[[:space:]]'

# A literal is allowed when it is under `.agent/`, or it is a transient location: an
# absolute `/tmp` path, a `.truenorth/` session dir, or a variable/interpolated path the
# lint cannot resolve. A literal is a violation when it names a repo-relative path that
# starts with a tracked top-level dir other than `.agent/` (most importantly `specs/`).
is_violation_literal() {
  literal="$1"
  # A variable or interpolated path is not a checkable literal. Skip it.
  case "$literal" in
    *'$'* | *'`'*) return 1 ;;
  esac
  # Allowed transient or machine locations.
  case "$literal" in
    /tmp/* | */.truenorth/* | .truenorth/* ) return 1 ;;
    .agent/* | */.agent/* ) return 1 ;;
  esac
  # A repo-relative write into `specs/` or another tracked top-level dir is a violation.
  case "$literal" in
    specs/* | ./specs/* | */specs/* ) return 0 ;;
    skills/* | ./skills/* ) return 0 ;;
    scripts/* | ./scripts/* ) return 0 ;;
    runtime/* | ./runtime/* ) return 0 ;;
    npm/* | ./npm/* ) return 0 ;;
    wiki/* | ./wiki/* ) return 0 ;;
  esac
  # Any other literal (a relative name with no tracked-dir prefix, an absolute path
  # outside the repo) is not flagged. The convention targets tracked repo writes.
  return 1
}

# Extract the first single- or double-quoted string literal from a line. The regex
# anchors on the first quote (`[^'\"]*` before it forbids an earlier quote), so a call
# with two literals, for example `writeFileSync('path', 'data')`, yields the first.
first_string_literal() {
  printf '%s\n' "$1" | sed -nE "s/^[^'\"]*['\"]([^'\"]*)['\"].*/\1/p" | head -n1
}

violations=0

for f in "${FILES[@]}"; do
  # A skill test fixture writes throwaway files, so a `tests/` path is out of the
  # artifact convention. Skip test files.
  case "$f" in
    */tests/* | *.test.js | *.test.cjs | *.test.mjs) continue ;;
  esac

  while IFS= read -r line_with_no; do
    lineno="${line_with_no%%:*}"
    line="${line_with_no#*:}"

    # A commented line carries no runtime write. Skip an obvious JS or shell comment.
    trimmed="$(printf '%s' "$line" | sed -E 's/^[[:space:]]*//')"
    case "$trimmed" in
      '//'* | '#'* | '*'* ) continue ;;
    esac

    literal="$(first_string_literal "$line")"
    [ -n "$literal" ] || continue

    if is_violation_literal "$literal"; then
      echo "lint-skill-artifacts: $f:$lineno writes a repo-relative path outside .agent/: '$literal'" >&2
      echo "  A machine artifact goes under .agent/; a transient render goes to /tmp or a gitignored session dir (ADR-0014)." >&2
      violations=$((violations + 1))
    fi
  done < <(grep -nE "$WRITE_CALL_RE|$PATH_CONST_RE|$SHELL_WRITE_RE" "$f" || true)
done

if [ "$violations" -gt 0 ]; then
  echo "lint-skill-artifacts: found $violations artifact-path violation(s). See ADR-0014." >&2
  exit 1
fi

echo "lint-skill-artifacts: all skill artifact-write targets obey the convention (ADR-0014)."
exit 0
