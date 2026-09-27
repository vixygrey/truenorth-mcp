#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: check-route.sh --mode <solo-git|team-pr> --action <inspect|create-pr|merge|tag|publish|cleanup> [--approved]
EOF
}

mode=""
action=""
approved=false

while [ "$#" -gt 0 ]; do
  case "$1" in
    --mode)
      [ "$#" -ge 2 ] || { usage; exit 2; }
      mode="$2"
      shift 2
      ;;
    --action)
      [ "$#" -ge 2 ] || { usage; exit 2; }
      action="$2"
      shift 2
      ;;
    --approved)
      approved=true
      shift
      ;;
    *)
      printf 'release route error: unknown argument %s.\n' "$1" >&2
      usage
      exit 2
      ;;
  esac
done

case "$mode" in
  solo-git | team-pr) ;;
  "")
    printf 'release route error: workflow_mode is missing. Set it to solo-git or team-pr in .agent/tasks/state.yml.\n' >&2
    exit 2
    ;;
  *)
    printf 'release route error: unsupported workflow_mode %s. Use solo-git or team-pr.\n' "$mode" >&2
    exit 2
    ;;
esac

case "$action" in
  inspect | create-pr | merge | tag | publish | cleanup) ;;
  "")
    printf 'release route error: --action is required.\n' >&2
    usage
    exit 2
    ;;
  *)
    printf 'release route error: unsupported action %s.\n' "$action" >&2
    usage
    exit 2
    ;;
esac

if ! command -v git >/dev/null 2>&1; then
  printf 'release route error: TrueNorth release workflows require Git. Install Git and use a Git repository. Jujutsu is not supported.\n' >&2
  exit 2
fi

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  printf 'release route error: the current directory is not a Git working tree. Jujutsu-only repositories are not supported.\n' >&2
  exit 2
fi

if [ "$mode" = "team-pr" ] && ! command -v gh >/dev/null 2>&1; then
  printf 'release route error: team-pr requires the GitHub CLI gh. Install and authenticate it, or explicitly configure workflow_mode: solo-git.\n' >&2
  exit 2
fi

case "$action" in
  merge | tag | publish | cleanup)
    if [ "$approved" != true ]; then
      printf 'release route error: %s is irreversible or destructive. Obtain explicit user approval for this action, then rerun with --approved.\n' "$action" >&2
      exit 3
    fi
    ;;
esac

printf '{"route":"%s","action":"%s","approved":%s}\n' "$mode" "$action" "$approved"
