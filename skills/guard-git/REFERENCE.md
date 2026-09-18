# Git guardrails — reference

## What the hook does

`scripts/block-dangerous-git.sh` reads a JSON command payload on stdin, extracts the
command, and blocks it when it matches a dangerous pattern from
`scripts/lib/git-guardrails-core.sh`:

- `git reset --hard`
- `git clean -fd`, `git clean -f`
- `git branch -D`
- `git checkout .`
- `git restore .`
- `git push --force`

The dangerous-pattern match is always on. The hook adds two opt-in policies.

`GIT_GUARDRAILS_MODE` selects the harness contract:

- `claude` (default) and `cursor`: print the reason to stderr and exit `2` on a block, exit
  `0` on allow.
- `gemini`: print a `{"decision": ...}` object on stdout and exit `0` always.

## Branch protection (opt-in)

Set `GIT_GUARDRAILS_PROTECT_BRANCH=1` to block a direct commit or push to `main` or
`master`. Set `GIT_GUARDRAILS_LAND=1` to bypass it for the deliberate land flow.

- A commit reads the current branch through `git rev-parse --abbrev-ref HEAD`.
- A push reads the target from the command (`git push origin main`, `git push origin
HEAD:main`), and falls back to the current branch.
- The hook reads git state for a commit or a push only. Every other command stays a string
  match. Outside a git repo the check fails open, so a command is allowed rather than
  blocked on a failed state read.

## Conventional Commits (opt-in)

Set `GIT_GUARDRAILS_CONVENTIONAL=1` to reject a `git commit -m` whose subject does not
match `type(scope): description` with an approved type. The approved types are `feat`,
`fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, and `revert`.

- The hook parses the quoted `-m`/`--message` subject from the command string.
- A `-F`/`--file` or heredoc message is skipped, because the subject is not on the command
  line. The `commit-msg` git hook validates those.
- A subject the hook cannot extract is skipped, so a legitimate commit is not blocked on a
  parse gap.

## Secret scanning (pre-commit hook)

The pre-command hook sees the command string only, not the staged diff. Secret scanning
lives in `scripts/pre-commit-secret-scan.sh`, a git `pre-commit` hook that scans `git diff
--cached` and blocks the commit on a match. Install it as the project `pre-commit` hook:

```bash
cp skills/guard-git/scripts/pre-commit-secret-scan.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

It blocks a staged diff that adds one of:

- `sk-` (OpenAI API keys)
- `ghp_` / `gho_` / `ghu_` / `ghs_` (GitHub tokens)
- `AKIA` (AWS access key id)
- `xoxb-` (Slack bot tokens)
- `-----BEGIN ... PRIVATE KEY-----` blocks

A match exits `1` and names the pattern and the fix. Bypass a false positive with `git
commit --no-verify`, and prefer redacting the value. The `audit-code` skill owns deeper
supply-chain review.

## Copy layout

Copy the script and its library, keeping `lib/` next to the script:

```text
<hooks-dir>/block-dangerous-git.sh
<hooks-dir>/lib/git-guardrails-core.sh
```

Example project locations:

- Claude: `.claude/hooks/`
- Cursor: `.cursor/hooks/`
- Gemini: `.gemini/hooks/`

Use the same layout for user-level hooks (`~/.claude/hooks`, `~/.cursor/hooks`,
`~/.gemini/hooks`).

---

## Claude Code

The hook command does **not** need `GIT_GUARDRAILS_MODE` (it defaults to `claude`).

**Project** (`.claude/settings.json`):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/block-dangerous-git.sh"
          }
        ]
      }
    ]
  }
}
```

---

## Cursor and Cursor CLI

Use `beforeShellExecution`. Set `GIT_GUARDRAILS_MODE=cursor`.

**Project** (`.cursor/hooks.json`):

```json
{
  "version": 1,
  "hooks": {
    "beforeShellExecution": [
      {
        "command": "GIT_GUARDRAILS_MODE=cursor .cursor/hooks/block-dangerous-git.sh",
        "matcher": "git"
      }
    ]
  }
}
```

---

## Gemini CLI

Use `BeforeTool` with matcher `run_shell_command`. Set **`GIT_GUARDRAILS_MODE=gemini`**.

**Project** (`.gemini/settings.json`):

```json
{
  "hooks": {
    "BeforeTool": [
      {
        "matcher": "run_shell_command",
        "hooks": [
          {
            "name": "git-guardrails",
            "type": "command",
            "command": "GIT_GUARDRAILS_MODE=gemini \"$GEMINI_PROJECT_DIR\"/.gemini/hooks/block-dangerous-git.sh",
            "timeout": 5000
          }
        ]
      }
    ]
  }
}
```

---

## Google Antigravity

Antigravity has no script hook. Add **Deny list** entries in
**Antigravity → Settings → Terminal** to mirror the dangerous patterns:

- `git push --force`
- `git reset --hard`
- `git clean`
- `git branch -D`
- `git checkout .`
- `git restore .`

---

## Verify (test harness)

Run the full test harness from the repository root. It covers every policy against the
shipped scripts and reports pass or fail:

```bash
bash skills/guard-git/scripts/tests/run.sh
# Expected: "23 passed, 0 failed", exit 0
```

The examples below run individual checks by hand. Run them from the directory that holds
`block-dangerous-git.sh` (with `lib/` beside it).

**1. Block a dangerous command (Claude mode):**

```bash
echo '{"tool_input":{"command":"git reset --hard HEAD~1"}}' | ./block-dangerous-git.sh
# Expected: exit 2, "BLOCKED: ... matches dangerous pattern ..." on stderr
```

**2. Block a force push (Claude mode):**

```bash
echo '{"tool_input":{"command":"git push --force origin main"}}' | ./block-dangerous-git.sh
# Expected: exit 2, blocked message on stderr
```

**3. Allow a safe command (Claude mode):**

```bash
echo '{"tool_input":{"command":"git status"}}' | ./block-dangerous-git.sh
# Expected: exit 0, no output
```

**4. Block a dangerous command (Gemini mode):**

```bash
echo '{"tool_input":{"command":"git clean -fd"}}' | GIT_GUARDRAILS_MODE=gemini ./block-dangerous-git.sh
# Expected: exit 0, {"decision":"deny","reason":"..."} on stdout
```

**5. Allow a safe command (Gemini mode):**

```bash
echo '{"tool_input":{"command":"git status"}}' | GIT_GUARDRAILS_MODE=gemini ./block-dangerous-git.sh
# Expected: exit 0, {"decision":"allow"} on stdout
```

**6. Block a push to main (branch protection):**

Run this from inside a git repository, on any branch.

```bash
echo '{"tool_input":{"command":"git push origin main"}}' | GIT_GUARDRAILS_PROTECT_BRANCH=1 ./block-dangerous-git.sh
# Expected: exit 2, "BLOCKED: a direct push to the protected branch 'main' ..." on stderr
```

**7. Block a non-conventional commit (Conventional Commits):**

```bash
echo '{"tool_input":{"command":"git commit -m \"update stuff\""}}' | GIT_GUARDRAILS_CONVENTIONAL=1 ./block-dangerous-git.sh
# Expected: exit 2, "BLOCKED: the commit subject 'update stuff' is not a Conventional Commit ..." on stderr
```

**8. Allow a conventional commit:**

```bash
echo '{"tool_input":{"command":"git commit -m \"feat: add a thing\""}}' | GIT_GUARDRAILS_CONVENTIONAL=1 ./block-dangerous-git.sh
# Expected: exit 0, no output
```

**9. Block a staged secret (pre-commit hook):**

Run this from inside a git repository with a secret staged.

```bash
printf 'token = "ghp_0123456789abcdefghijABCDEF"\n' > cfg.txt && git add cfg.txt
skills/guard-git/scripts/pre-commit-secret-scan.sh
# Expected: exit 1, "BLOCKED: the staged diff contains a GitHub token pattern ..." on stderr
```
