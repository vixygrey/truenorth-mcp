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

Any other command is allowed. The hook inspects the command string only. It does not run
git, read the repository, or check the branch, so it does not enforce branch protection,
Conventional Commits, or secret scanning.

`GIT_GUARDRAILS_MODE` selects the harness contract:

- `claude` (default) and `cursor`: print the reason to stderr and exit `2` on a block, exit
  `0` on allow.
- `gemini`: print a `{"decision": ...}` object on stdout and exit `0` always.

## Secret hygiene (advisory)

The hook does not scan for secrets. As a separate practice, do not commit files containing:

- `sk-` (OpenAI API keys)
- `ghp_` / `gho_` (GitHub tokens)
- `AKIA` (AWS access key id)
- `xoxb-` (Slack bot tokens)
- `-----BEGIN` private keys

Use the `audit-code` supply-chain checklist before a commit. Consider `git-secrets` or a
dedicated pre-commit hook in the target project.

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

## Verify (local tests)

Run these from the directory that holds `block-dangerous-git.sh` (with `lib/` beside it).

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
