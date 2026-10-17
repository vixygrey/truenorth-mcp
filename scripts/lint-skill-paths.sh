#!/usr/bin/env bash
# Lint skill and wiki cockpit paths against the `.agent/` layout (issues #250, #452).
#
# A skill document names `.agent/` paths in its prose and worked examples. The runtime
# relocated the cockpit under `.agent/` (ADR-0011), and the write guard confines every
# runtime write to `.agent/` (ADR-0008). A document that names a retired path, an
# unqualified cockpit filename, or an unknown `.agent/` area is drift.
#
# The lint validates by area membership, not by exact file existence. A worked example
# can name a fictional path under a valid area. A relative layout entry or legacy
# migration-source example can name a bare cockpit filename only when its line carries
# one of these explicit markers:
#   truenorth-lint: allow-relative-layout-path
#   truenorth-lint: allow-legacy-cockpit-path
#
# Usage: bash scripts/lint-skill-paths.sh [--root REPOSITORY]

set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$SCRIPT_ROOT"
if [ "${1:-}" = "--root" ]; then
  if [ "$#" -ne 2 ] || [ ! -d "$2" ]; then
    echo "lint-skill-paths: --root requires an existing repository directory." >&2
    exit 2
  fi
  REPO_ROOT="$(cd "$2" && pwd)"
elif [ "$#" -ne 0 ]; then
  echo "lint-skill-paths: usage: bash scripts/lint-skill-paths.sh [--root REPOSITORY]" >&2
  exit 2
fi
cd "$REPO_ROOT"

# The known top-level areas under `.agent/`, from the agent-workspace-profiles design §1
# and ADR-0011. A path directly under `.agent/` names one of these areas, or the
# `ontology.yml` file at the `.agent/` root.
KNOWN_AREAS="config spec tasks product memories telemetry runtime"
ROOT_FILES="ontology.yml layout.yml profile.yml"

# A retired cockpit path a skill must no longer name. These are the drift the lint exists
# to catch: the pre-relocation `specs/` cockpit files and the pre-rename `.bigpowers/`
# dir. `specs/adr/` stays legal, because the ADR resource reads it (ADR-0011).
RETIRED_RE='(^|[^A-Za-z0-9_./-])(specs/(state|release-plan|ontology|tasks|product|backlog|active-feature)|\.bigpowers)([/.]|$)'

# Runtime-owned cockpit documents use JSON. The explicit marker is narrow by design:
# it permits one labeled legacy-migration-source line, not a whole file or directory.
RETIRED_COCKPIT_YAML_RE='(\.agent/tasks/(state|release-plan|bugs)\.ya?ml|(^|[^A-Za-z0-9_./-])(state|release-plan|bugs)\.ya?ml([^A-Za-z0-9_.-]|$))'
ALLOW_MARKER_RE='truenorth-lint: allow-(legacy-cockpit|relative-layout)-path'

# The source skill documents and in-repo wiki. The packaged mirror is checked separately
# for byte identity with `skills/`, so scanning it here would only duplicate diagnostics.
mapfile -t FILES < <(git ls-files 'skills/**/*.md' 'wiki/*.md')
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "lint-skill-paths: no skill or wiki documents to lint."
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
  [ -f "$f" ] || continue
  # Check retired cockpit paths line by line, so the report names the line.
  while IFS= read -r hit; do
    lineno="${hit%%:*}"
    text="${hit#*:}"
    if printf '%s\n' "$text" | grep -Eq "$ALLOW_MARKER_RE"; then
      continue
    fi
    echo "lint-skill-paths: $f:$lineno names a retired cockpit path: ${text#"${text%%[![:space:]]*}"}" >&2
    echo "  The cockpit lives under .agent/ (ADR-0011); specs/ cockpit files and .bigpowers/ are retired." >&2
    violations=$((violations + 1))
  done < <(grep -nE "$RETIRED_RE" "$f" || true)

  # YAML is retired only for the three runtime-owned cockpit documents.
  while IFS= read -r hit; do
    lineno="${hit%%:*}"
    line="${hit#*:}"
    if printf '%s\n' "$line" | grep -Eq "$ALLOW_MARKER_RE"; then
      continue
    fi
    if printf '%s\n' "$line" | grep -Eq "$RETIRED_COCKPIT_YAML_RE"; then
      echo "lint-skill-paths: $f:$lineno names a retired YAML cockpit path: ${line#"${line%%[![:space:]]*}"}" >&2
      echo "  Use .agent/tasks/<name>.json, or mark one clearly labeled legacy migration-source line." >&2
      violations=$((violations + 1))
    fi
  done < <(grep -nE "$RETIRED_COCKPIT_YAML_RE" "$f" || true)

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

echo "lint-skill-paths: every skill and wiki cockpit path uses a known .agent/ area."
exit 0
