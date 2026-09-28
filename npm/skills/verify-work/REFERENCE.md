# Verify Work — Reference

## Cold-start smoke

```bash
# Example — adapt to the project agent guide
pkill -f "<dev-server>" 2>/dev/null || true
rm -rf .next/cache node_modules/.cache 2>/dev/null || true
<run command> &
sleep 3 && curl -sf http://localhost:<port>/health || echo "BOOT FAIL"
```

## Gaps template

```markdown
## Gaps (verify-work)

| Step | Expected | Actual | Status |
| ---- | -------- | ------ | ------ |
| 1    | ...      | ...    | FAIL   |
```

Feed gaps to `plan-work` as new steps with verify commands, then re-run verify-work.

## Evidence template

```yaml
task_id: task-17
verified_at: "2026-06-11T14:30:00Z"
risk: P1
verifier: verify-work
phases:
  behavior_smoke:
    command: "npm run smoke -- login"
    expected: "Login form renders"
    actual: "Login form rendered correctly"
    passed: true
  tests:
    command: "npm test"
    passed: true
  terminal_verdict:
    command: "npm test"
    exit_code: 0
    captured_at: "2026-06-11T14:25:00Z"
  manual:
    required: true
    reason: "Acceptance criteria require visual judgment"
    steps:
      - step: "Open /login"
        expected: "Login form renders"
        actual: "Login form rendered correctly"
        passed: true
  gaps:
    closed: true
```

## CLI mode

For CLI tools where cold-start smoke does not apply. Auto-detected when no server process; or use `--cli`.

**Auto-detect binary name:**

```bash
BINARY=$(grep '^name' Cargo.toml | head -1 | awk -F'"' '{print $2}')  # Cargo
BINARY=$(node -e "console.log(require('./package.json').bin && Object.keys(require('./package.json').bin)[0] || '')" 2>/dev/null)
BINARY=$(grep '^BIN\s*=' Makefile 2>/dev/null | awk '{print $3}')
```

**Checklist (replaces cold-start smoke):**

1. `$BINARY --help` → output contains "Usage"
2. `$BINARY --version` → matches manifest
3. README example command → non-empty output
4. `$BINARY --invalid-flag` → exit ≠ 0 with error message
