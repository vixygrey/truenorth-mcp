#!/usr/bin/env bash
# Lint the cockpit paths a skill names against the `.agent/` layout (issue #250).
#
# A skill document names `.agent/` paths in its prose and its worked examples. The
# runtime relocated the cockpit under `.agent/` (ADR-0011), and the write guard confines
# every runtime write to `.agent/` (ADR-0008). A skill that still names a retired path,
# for example a `specs/` cockpit file or a `.bigpowers/` dir, or that names a `.agent/`
# path under an area the layout does not define, is drift (the #165 to #168 class).
#
# The lint validates by AREA MEMBERSHIP, not by exact file existence. A worked example
# names a fictional path, for example `.agent/tasks/e02-auth-ui/story.yml`, so a
# file-exists check would flag every tutorial. Instead the lint confirms the first path
# segment under `.agent/` is a known area, and rejects a retired cockpit path outright.
#
# Usage: bash scripts/lint-skill-paths.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The known top-level areas under `.agent/`, from the agent-workspace-profiles design §1
# and ADR-0011. A path directly under `.agent/` names one of these areas, or the
# `ontology.yml` file at the `.agent/` root.
KNOWN_AREAS="config spec tasks product memories telemetry"
ROOT_FILES="ontology.yml layout.yml profile.yml"

# A retired cockpit path a skill must no longer name. These are the drift the lint exists
# to catch: the pre-relocation `specs/` cockpit files and the pre-rename `.bigpowers/`
# dir. `specs/adr/` stays legal, because the ADR resource reads it (ADR-0011).
RETIRED_RE='(^|[^A-Za-z0-9_./-])(specs/(state|release-plan|ontology|tasks|product|backlog|active-feature)|\.bigpowers)([/.]|$)'

# The tracked skill documents. A git glob, not a shell glob, so detection matches CI.
mapfile -t FILES < <(git ls-files 'skills/**/*.md')
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "lint-skill-paths: no skill documents to lint."
  exit 0
fi

# Return 0 when the first `.agent/` path segment is a known area or a known root file.
is_known_agent_path() {
  rest="$1" # the text after `.agent/`
  # A root file: `.agent/ontology.yml`, `.agent/layout.yml`, `.agent/profile.yml`.
  for rf in $ROOT_FILES; do
    [ "$rest" = "$rf" ] && return 0
  done
  # An area path: the segment before the first `/`, or the whole rest when no `/`.
  seg="${rest%%/*}"
  for area in $KNOWN_AREAS; do
    [ "$seg" = "$area" ] && return 0
  done
  return 1
}

violations=0

for f in "${FILES[@]}"; do
  # Check retired cockpit paths line by line, so the report names the line.
  while IFS= read -r hit; do
    lineno="${hit%%:*}"
    text="${hit#*:}"
    echo "lint-skill-paths: $f:$lineno names a retired cockpit path: ${text#"${text%%[![:space:]]*}"}" >&2
    echo "  The cockpit lives under .agent/ (ADR-0011); specs/ cockpit files and .bigpowers/ are retired." >&2
    violations=$((violations + 1))
  done < <(grep -nE "$RETIRED_RE" "$f" || true)

  # Check every `.agent/` path token for a known area.
  while IFS= read -r hit; do
    lineno="${hit%%:*}"
    # Extract each `.agent/<...>` token on the line.
    line="${hit#*:}"
    while IFS= read -r token; do
      [ -n "$token" ] || continue
      rest="${token#.agent/}"
      # A bare `.agent` or `.agent/` reference names the root, which is always known.
      [ -z "$rest" ] && continue
      if ! is_known_agent_path "$rest"; then
        seg="${rest%%/*}"
        echo "lint-skill-paths: $f:$lineno names an unknown .agent/ area: '$token' (area '$seg')" >&2
        echo "  Known areas: $KNOWN_AREAS. Root files: $ROOT_FILES (see .agent/layout.yml, ADR-0011)." >&2
        violations=$((violations + 1))
      fi
    done < <(printf '%s\n' "$line" | grep -oE '\.agent/[A-Za-z0-9_./-]+' | sed -E 's/[.,)`:]+$//')
  done < <(grep -nE '\.agent/[A-Za-z0-9_./-]+' "$f" || true)
done

if [ "$violations" -gt 0 ]; then
  echo "lint-skill-paths: found $violations skill-path violation(s) against the .agent/ layout." >&2
  exit 1
fi

echo "lint-skill-paths: every skill names cockpit paths under a known .agent/ area."
exit 0
