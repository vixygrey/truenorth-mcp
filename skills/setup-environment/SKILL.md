---
name: setup-environment
description: Pre-install dependencies and configure tools before development begins. Use it at session start on a fresh clone, before kickoff-branch, or when the user says "setup environment" or "install deps".
kind: prose
---

# Setup Environment

> **HARD GATE**: environment setup MUST be idempotent and reproducible. When setup fails, give a clear error message and remediation steps. Do NOT assume a prior state.

Idempotent prep, so the build-phase commands succeed on the first run.

## Checklist

1. Read the project agent guide and conventions for the required runtimes and
   commands.
2. Verify the runtime versions (`node -v`, `swift --version`).
3. Install the dependencies (`npm ci`, `bundle install`). Prefer a lockfile install.
4. Copy `.env.example` to `.env` when documented. Never commit a secret.
5. Run a smoke check: lint plus one fast test, or `--version` on the key tools.
6. Record the versions in `.agent/tasks/state.yml` under the environment section.

## Verify

Confirm the runtimes and the key tool versions resolve. A pass means every required
command is present and reports its version.
