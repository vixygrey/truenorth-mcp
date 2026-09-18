---
name: guard-git
description: Block a dangerous git command (force push, reset --hard, clean, branch -D, checkout or restore of a path) before an agent runs it. Install a pre-command hook for the agent harness in use. Use it when the user wants git-safety hooks, to block a destructive git command in an agent, or to mirror the same policy across coding tools.
---

# Guard Git

> **HARD GATE**: this hook blocks dangerous git commands. Branch protection and
> Conventional Commits are opt-in and off by default. Secret scanning is a separate
> pre-commit hook. Before you rely on any check, run the test harness and confirm the
> author and the git user are configured. A bad commit is hard to fix.

Install a shared hook that blocks a destructive git command before an agent runs it.
The hook needs `jq` on the PATH when it runs.

## What gets blocked

The hook always blocks a command that matches a dangerous pattern:

- `git reset --hard`
- `git clean -fd`, `git clean -f`
- `git branch -D`
- `git checkout .`
- `git restore .`
- `git push --force`

The hook adds two opt-in policies, off by default. Each policy is a single environment
flag:

- **Branch protection** (`GIT_GUARDRAILS_PROTECT_BRANCH=1`): block a direct commit or push
  to `main` or `master`. Set `GIT_GUARDRAILS_LAND=1` to bypass it for the deliberate land
  flow. The hook reads the current branch for a commit or a push only. Outside a git repo
  the check fails open, so the hook never hard-fails on a state read.
- **Conventional Commits** (`GIT_GUARDRAILS_CONVENTIONAL=1`): reject a `git commit -m`
  whose subject does not match `type(scope): description` with an approved type. A
  `-F`/`--file` or heredoc message is skipped, because the subject is not on the command
  line. The `commit-msg` git hook validates those.

Any command the three checks do not block is allowed.

## Secret scanning (pre-commit)

The pre-command hook sees the command string only, not the staged diff, so secret
scanning lives in a separate `pre-commit` hook, `scripts/pre-commit-secret-scan.sh`. It
scans `git diff --cached` for a common secret and blocks the commit. The `audit-code`
skill owns deeper supply-chain review. See [REFERENCE.md](REFERENCE.md) for install and
verify steps.

## Quick start

1. **Scope**: ask project-only versus global. The paths differ per tool.
2. **Copy** `scripts/block-dangerous-git.sh` and `scripts/lib/git-guardrails-core.sh` into
   the harness hooks directory, keeping the `lib/` subdirectory next to the script.
3. **Make it executable** with `chmod +x` on `block-dangerous-git.sh`.
4. **Merge** the hook snippet from [REFERENCE.md](REFERENCE.md) into the correct settings
   file. Do not wipe an unrelated key.
5. **Enable a policy** by setting `GIT_GUARDRAILS_PROTECT_BRANCH=1` or
   `GIT_GUARDRAILS_CONVENTIONAL=1` in the hook command, when you want it.
6. **Install the secret scanner** by copying `scripts/pre-commit-secret-scan.sh` to the
   project `pre-commit` hook, when you want staged-diff secret scanning.
7. **Verify** by running the test harness:

   ```bash
   bash skills/guard-git/scripts/tests/run.sh
   ```

The hook mechanism is harness-specific. The policy is identical across harnesses.
This table records the pre-command hook point for common harnesses, as reference.
The skill does not depend on any one harness.

| Harness                             | Mechanism                                               | Config                             |
| ----------------------------------- | ------------------------------------------------------- | ---------------------------------- |
| Pre-command hook (generic)          | A before-shell-execution hook that inspects the command | The harness hooks or settings file |
| A hook that blocks on non-zero exit | stderr plus exit `2` on a block                         | The harness hooks file             |
| A hook that reads a JSON decision   | A `decision` object on stdout                           | The harness settings file          |

Set the hook mode to match the harness contract: a stderr-plus-exit-2 block, or a
JSON decision on stdout.

## Customization

The policy data lives in `scripts/lib/git-guardrails-core.sh`:

- Add or remove a dangerous pattern in `GIT_GUARDRAILS_PATTERNS`.
- Change the protected branch names in `GIT_GUARDRAILS_PROTECTED_BRANCHES`.
- Change the approved commit types in `GIT_GUARDRAILS_CC_TYPES`.

The secret patterns live in `scripts/pre-commit-secret-scan.sh` in `SECRET_PATTERNS` and
their labels in `SECRET_LABELS`.

## Advanced

Full JSON examples, merge rules, and test commands: [REFERENCE.md](REFERENCE.md).
