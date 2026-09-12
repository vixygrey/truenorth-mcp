---
name: guard-git
description: Block a dangerous git command (push, force push, reset --hard, clean, branch -D, checkout or restore of a path) and enforce Conventional Commits and branch protection before an agent runs it. Install a pre-command hook for the agent harness in use. Use it when the user wants git-safety hooks, to block a destructive git command in an agent, or to mirror the same policy across coding tools.
---

# Guard Git

> **HARD GATE**: before committing, verify the branch is not `main` or `master`, the author is correct, and the git user is configured. A bad commit is hard to fix.

Install a shared hook that blocks a destructive git operation and enforces workflow
discipline. The hook needs `jq` on the PATH when it runs.

## What gets blocked or enforced

- **Safety**: `git push --force`, `git reset --hard`, `git clean -f`, `git branch -D`, `git checkout .`, `git restore .`.
- **Discipline**: block a direct commit or push to a protected branch (`main`, `master`), except the deliberate solo land to `main`.
- **Allow**: `git push origin <feature-branch>` for backup or CI.
- **Standardization**: enforce Conventional Commits for every `git commit`.
- **Secrets**: block a commit that contains a common secret pattern (`sk-`, `ghp_`, `AKIA`, `xoxb-`, a `-----BEGIN` private key). See [REFERENCE.md](REFERENCE.md).

## Quick start

1. **Scope**: ask project-only versus global. The paths differ per tool.
2. **Write the hook bundle** from [REFERENCE.md](REFERENCE.md) into the harness hooks directory.
3. **Make it executable** with `chmod +x` on the hook script.
4. **Merge** the hook snippet into the correct settings file. Do not wipe an unrelated key.
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

To add or remove a pattern or a protected branch, edit the hook script.

## Advanced

Full JSON examples, merge rules, and test commands: [REFERENCE.md](REFERENCE.md).
