---
name: commit-message
description: Review the working-tree changes, draft a Conventional Commits title and body, and state the SemVer bump a single such commit would imply. Also note which defensive-code categories were touched. Use it when the user wants to commit recent work or prepare a Conventional Commits message before a git commit.
kind: prose
---

# Commit Message

> **HARD GATE**: a commit MUST follow the Conventional Commits format, `type(scope): description`. Do NOT use a vague message like "fix" or "updates". The message explains the why, not the what.

## Modes

- Default: a standard Conventional Commits message.
- `--fix-type`: force `type=fix`. Use it when the commit type is unambiguous.

## What "recent work" means

- **Primary source of truth**: read the `vcs.kind` value from `state.yaml`. Git
  uses `git status`, `git diff`, and `git diff --cached`. Jujutsu uses
  `jj status`, `jj diff`, and `jj log -r @`. Run in the repo root.
- **Context**: use the current conversation to summarize the intent and to spot a
  breaking API or behavior change that the diff alone may not show.
- When the user tracks a session baseline, you can diff from it plus the
  uncommitted changes. Otherwise use the index and the working tree.

## Quick workflow

1. **Inventory**: list the changed paths. Group them by feature, chore, docs, or
   test-only.
2. **Decide the commit shape**: one atomic commit is ideal. When the diff mixes
   unrelated concerns, recommend multiple commits, each with its own type and
   scope, before you suggest one message.
3. **Classify for SemVer**: `fix` is a patch, `feat` is a minor, a breaking change
   is a major.
4. **Write the message**: `type(optional-scope)!: description`. Use `!` or a
   `BREAKING CHANGE:` footer when a behavior contract changes. See
   [REFERENCE.md](REFERENCE.md#message-format).
5. **Note the defensive-code categories touched**: rate limit, retry with backoff,
   circuit breaker, timeout, graceful degradation.
6. **Deliver**:
   - The proposed full commit message (title, optional body, footers).
   - The release bump this commit would drive: patch, minor, major, or none.
   - The optional native command: Git `git commit -m`, Jujutsu `jj commit -m`.
     Never omit `-m`, and never run a destructive command unless asked.

## Checklist before finalizing

- [ ] The type matches the dominant user-visible outcome (`feat` versus `fix` versus `perf`).
- [ ] The scope is a short noun in parentheses when it helps, for example `fix(api): ...`.
- [ ] A breaking change is explicit (`!` or a `BREAKING CHANGE:` footer).
- [ ] The description is imperative, lowercase after the prefix, with no trailing period on the title line.
- [ ] No `Co-authored-by` footer. Every commit appears authored solely by the human user.

## Release mapping

The bump follows the commit type. The release itself is tag-driven: a `v*` tag
triggers the release workflow. The commit message does not publish on its own. See
`.github/COMMIT_TEMPLATE.md` for the full convention.

## Further reading

- [REFERENCE.md](REFERENCE.md): the message shape, footers, release mapping, and squashing notes.

## Handoff

Gate: READY. Next: release-branch.
Writes: `state.yaml` `handoff.next_skill = release-branch`.
