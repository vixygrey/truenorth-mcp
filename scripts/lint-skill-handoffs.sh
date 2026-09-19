#!/usr/bin/env bash
# Lint skill handoff targets against the catalog (issue #253).
#
# A skill hands off to the next skill by declaring a target, for example
# `handoff.next_skill = verify-work` or a `Gate: READY. Next: verify-work.` line. A
# handoff to a skill that no longer exists is a dangling reference (the P5 class): a
# rename or a delete leaves the pointer stale. This lint confirms every concrete handoff
# target names a real skill directory.
#
# The lint is deliberately conservative. It flags only a concrete kebab-case target after
# `next_skill` or a `Next:` line. A generic reference such as `handoff.next_skill` in
# backticks with no name, or `next_skill: skill for the current phase`, names no skill, so
# the lint does not flag it.
#
# Usage: bash scripts/lint-skill-handoffs.sh

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || exit 1

# The tracked skill documents. A git glob, not a shell glob, so detection matches CI.
mapfile -t FILES < <(git ls-files 'skills/**/*.md')
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "lint-skill-handoffs: no skill documents to lint."
  exit 0
fi

# A concrete handoff target is a kebab-case token after one of these forms:
#   next_skill: <target>        next_skill = <target>
#   Next: <target>              (a `Gate: ... Next: <target>` prose line)
# The target is the first kebab-case word that follows. A word that is not kebab-case, or
# text in backticks with no name, yields no target and is not flagged.
#
# `NAME_RE` is a plausible skill name: two-or-more kebab-case words. It matches `verify-work`
# and `develop-tdd`, and it does not match a single prose word such as `skill`, so a generic
# `next_skill: skill for the current phase` reference does not produce a false target.
NAME_RE='[a-z][a-z0-9]*(-[a-z0-9]+)+'

# Extract concrete targets from `next_skill` forms. Strip an optional leading backtick or
# space, require a `:` or `=` separator, then capture the kebab-case name.
extract_next_skill() {
  sed -nE "s/.*next_skill[\`]?[[:space:]]*[:=][[:space:]]*[\`\"']?($NAME_RE).*/\1/p"
}

# Extract concrete targets from a `Next: <name>` prose line. Anchor on `Next:` so the
# `next_skill` token above does not also match here.
extract_next_line() {
  sed -nE "s/.*[^_]Next:[[:space:]]*($NAME_RE).*/\1/p; s/^Next:[[:space:]]*($NAME_RE).*/\1/p"
}

violations=0

for f in "${FILES[@]}"; do
  while IFS= read -r hit; do
    lineno="${hit%%:*}"
    line="${hit#*:}"

    # Collect every concrete target on the line, from both forms.
    targets=$(
      {
        printf '%s\n' "$line" | extract_next_skill
        printf '%s\n' "$line" | extract_next_line
      } | sort -u
    )

    for target in $targets; do
      [ -n "$target" ] || continue
      if [ ! -f "skills/$target/SKILL.md" ]; then
        echo "lint-skill-handoffs: $f:$lineno hands off to an unknown skill: '$target'" >&2
        echo "  No skills/$target/SKILL.md. A handoff must name a real skill (rename or fix the target)." >&2
        violations=$((violations + 1))
      fi
    done
  done < <(grep -nE 'next_skill|Next:' "$f" || true)
done

if [ "$violations" -gt 0 ]; then
  echo "lint-skill-handoffs: found $violations dangling handoff target(s)." >&2
  exit 1
fi

echo "lint-skill-handoffs: every concrete handoff target names a real skill."
exit 0
