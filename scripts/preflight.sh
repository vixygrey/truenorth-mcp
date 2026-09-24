#!/usr/bin/env bash
# Run the same verification groups as the required CI gate (issue #381).
#
# Usage:
#   bash scripts/preflight.sh
#   bash scripts/preflight.sh check|wrapper|artifact-smoke|format|shell [...]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

TEMP_DIRS=()
cleanup() {
  local directory
  for directory in "${TEMP_DIRS[@]}"; do
    rm -rf "$directory"
  done
}
trap cleanup EXIT

fail() {
  echo "preflight: $*" >&2
  exit 1
}

require_command() {
  local command_name=$1
  local remediation=$2
  command -v "$command_name" >/dev/null 2>&1 || \
    fail "required command '$command_name' is unavailable. $remediation"
}

require_bash_four() {
  if ((BASH_VERSINFO[0] < 4)); then
    fail "Bash 4 or newer is required; found ${BASH_VERSION}. Install a current Bash and rerun this command with it."
  fi
}

run_check() {
  require_command cargo "Install the stable Rust toolchain with rustfmt and clippy."
  cargo fmt --version >/dev/null 2>&1 || \
    fail "rustfmt is unavailable. Install it with 'rustup component add rustfmt'."
  cargo clippy --version >/dev/null 2>&1 || \
    fail "clippy is unavailable. Install it with 'rustup component add clippy'."

  echo "preflight: [check] cargo fmt"
  cargo fmt --manifest-path runtime/Cargo.toml -- --check
  echo "preflight: [check] cargo clippy"
  cargo clippy --manifest-path runtime/Cargo.toml --all-targets -- -D warnings
  echo "preflight: [check] cargo build"
  cargo build --manifest-path runtime/Cargo.toml --verbose
  echo "preflight: [check] cargo test"
  cargo test --manifest-path runtime/Cargo.toml --verbose
  echo "preflight: [check] cargo clippy with tree-sitter"
  cargo clippy --manifest-path runtime/Cargo.toml --features tree-sitter --all-targets -- -D warnings
  echo "preflight: [check] cargo test with tree-sitter"
  cargo test --manifest-path runtime/Cargo.toml --features tree-sitter --verbose
}

run_wrapper() {
  require_command node "Install Node.js 22 or newer."
  echo "preflight: [wrapper] node tests"
  (cd npm && node --test)
}

platform_package() {
  local platform architecture package
  platform=$(node -p 'process.platform')
  architecture=$(node -p 'process.arch')
  case "$platform/$architecture" in
    darwin/arm64) package=darwin-arm64 ;;
    darwin/x64) package=darwin-x64 ;;
    linux/arm64) package=linux-arm64 ;;
    linux/x64) package=linux-x64 ;;
    *)
      fail "artifact smoke is unavailable on $platform/$architecture. Supported platforms are darwin-arm64, darwin-x64, linux-arm64, and linux-x64."
      ;;
  esac
  printf '%s\n' "$package"
}

run_artifact_smoke() {
  require_command cargo "Install the stable Rust toolchain."
  require_command node "Install Node.js 22 or newer."
  require_command npm "Install npm with Node.js."

  local package expected_version release_binary stage_dir
  package=$(platform_package)
  expected_version=$(node -p "require('./npm/package.json').version")
  release_binary=runtime/target/release/truenorth-mcp

  echo "preflight: [artifact-smoke] release record tests"
  node --test scripts/release-record.test.js
  echo "preflight: [artifact-smoke] release build for $package"
  cargo build --release --manifest-path runtime/Cargo.toml

  stage_dir=$(mktemp -d "${TMPDIR:-/tmp}/truenorth-preflight.XXXXXX")
  TEMP_DIRS+=("$stage_dir")
  cp "npm/packages/$package/package.json" "$stage_dir/package.json"
  cp "$release_binary" "$stage_dir/truenorth-mcp"
  chmod +x "$stage_dir/truenorth-mcp"

  echo "preflight: [artifact-smoke] native artifact"
  node scripts/smoke-mcp-artifact.js \
    --expected-version "$expected_version" \
    -- "$stage_dir/truenorth-mcp"
  echo "preflight: [artifact-smoke] packed wrapper artifact"
  node scripts/smoke-mcp-artifact.js \
    --expected-version "$expected_version" \
    --packed-wrapper "$stage_dir"
}

run_format() {
  require_command node "Install Node.js 22 or newer."
  require_command npm "Install npm with Node.js."
  if [ ! -x node_modules/.bin/prettier ]; then
    fail "root npm development dependencies are unavailable. Run 'npm install --no-audit --no-fund'."
  fi

  echo "preflight: [format] prettier"
  npm run format:check
  echo "preflight: [format] verify-gate documentation"
  npm run lint:verify-gate-docs
}

run_shell() {
  require_bash_four
  require_command git "Install Git."

  echo "preflight: [shell] shellcheck"
  bash scripts/lint-shell.sh
  echo "preflight: [shell] skill artifact paths"
  bash scripts/lint-skill-artifacts.sh
  echo "preflight: [shell] skill cockpit paths"
  bash scripts/lint-skill-paths.sh
  echo "preflight: [shell] skill handoffs"
  bash scripts/lint-skill-handoffs.sh
  echo "preflight: [shell] scripted skills"
  bash scripts/verify-skills.sh
}

run_group() {
  case "$1" in
    check) run_check ;;
    wrapper) run_wrapper ;;
    artifact-smoke) run_artifact_smoke ;;
    format) run_format ;;
    shell) run_shell ;;
    *) fail "unknown group '$1'. Expected check, wrapper, artifact-smoke, format, or shell." ;;
  esac
}

require_bash_four
if (($# == 0)); then
  groups=(check wrapper artifact-smoke format shell)
else
  groups=("$@")
fi

for group in "${groups[@]}"; do
  run_group "$group"
done

echo "preflight: all requested groups passed."
