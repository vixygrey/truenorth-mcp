---
name: guard-git
description: Block a dangerous git command (force push, reset --hard, clean, branch -D, checkout or restore of a path) before an agent runs it. Install a pre-command hook for the agent harness in use. Use it when the user wants git-safety hooks, to block a destructive git command in an agent, or to mirror the same policy across coding tools.
---

# Guard Git

> **HARD GATE**: this hook blocks dangerous git commands only. It does not check the
> branch, the author, or the commit message. Before committing, verify by hand that the
> branch is not `main` or `master`, the author is correct, and the git user is configured.
> A bad commit is hard to fix.

Install a shared hook that blocks a destructive git command before an agent runs it.
The hook needs `jq` on the PATH when it runs.

## What gets blocked

The hook blocks a command that matches a dangerous pattern:

- `git reset --hard`
- `git clean -fd`, `git clean -f`
- `git branch -D`
- `git checkout .`
- `git restore .`
- `git push --force`

Any other command is allowed. The hook inspects the command string only; it does not run
git or read the repository, so it does not enforce branch protection, Conventional
Commits, or secret scanning. For those, see the advisory guidance in
[REFERENCE.md](REFERENCE.md) and the `audit-code` skill.

## Quick start

1. **Scope**: ask project-only versus global. The paths differ per tool.
2. **Copy** `scripts/block-dangerous-git.sh` and `scripts/lib/git-guardrails-core.sh` into
   the harness hooks directory, keeping the `lib/` subdirectory next to the script.
3. **Make it executable** with `chmod +x` on `block-dangerous-git.sh`.
4. **Merge** the hook snippet from [REFERENCE.md](REFERENCE.md) into the correct settings
   file. Do not wipe an unrelated key.
5. **Verify** with the tests in [REFERENCE.md](REFERENCE.md).

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

To add or remove a dangerous pattern, edit `GIT_GUARDRAILS_PATTERNS` in
`scripts/lib/git-guardrails-core.sh`.

## Advanced

Full JSON examples, merge rules, and test commands: [REFERENCE.md](REFERENCE.md).
