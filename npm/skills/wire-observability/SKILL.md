---
name: wire-observability
description: Add structured JSON logging, observability commands, and idempotent setup scripts to a project. Use it when a project needs production-readiness instrumentation, when the user wants structured logging, or as a production-readiness gate at any phase.
kind: prose
---

# Wire Observability

> **HARD GATE**: observability is not optional. Before shipping, verify that structured logging is in place, the key metrics are instrumented, and an error case emits a signal. "We will add metrics later" becomes "never".

Add structured logging, observability commands, and idempotent setup scripts. Invoke
it at any phase. It is recommended at the end of the first working slice, before the
first deploy.

## What this sets up

1. Structured JSON logging: machine-readable logs for debugging and observability.
2. Observability commands: how to check the system health, documented in the project
   agent guide.
3. Idempotent setup scripts: a script that runs repeatedly without a side effect.

## Process

### 1. Assess the current state

Check what is already in place.

- Is there a logging library (pino, winston, structlog, zap, slog)?
- Is the logging JSON or plain text?
- Is there a health-check endpoint or command?
- Are there setup scripts, and are they idempotent?

### 2. Add structured JSON logging

For user-facing CLI output, plain text is fine. For everything else, use structured
JSON.

```json
{
  "level": "info",
  "timestamp": "2025-01-15T10:23:45.123Z",
  "message": "User created",
  "userId": "usr_abc123",
  "requestId": "req_xyz789"
}
```

Guidelines:

- Include `level`, `timestamp`, and `message` in every entry.
- Add the context fields relevant to the operation (userId, requestId, traceId).
- Log at the boundaries: HTTP requests in and out, DB queries, external API calls,
  and background-job start and end.
- Log an error with its stack trace.
- Never log a secret, a password, a token, or PII.

### 3. Document the observability commands

Add an observability section to the project agent guide.

```markdown
## Observability

| What                    | Command                  |
| ----------------------- | ------------------------ |
| View the logs           | `<log tail command>`     |
| Health check            | `<health check command>` |
| Check the DB connection | `<db ping command>`      |
| View the metrics        | `<metrics command>`      |
```

### 4. Write idempotent setup scripts

An idempotent script runs multiple times and always produces the same result, with
no error on a re-run. Check whether the thing exists before you create it.

```bash
#!/usr/bin/env bash
set -euo pipefail

if ! psql -c "SELECT 1 FROM pg_database WHERE datname = 'myapp'" | grep -q 1; then
  createdb myapp
  echo "Database created"
else
  echo "Database already exists, skipping"
fi
```

Place the setup script in the project's conventional scripts location, and document
the command in the project agent guide.

### 5. Verify

- [ ] Run the app and confirm the JSON logs appear in the correct format.
- [ ] Run the setup script twice. The second run produces no error.
- [ ] The health-check command returns success.
- [ ] No sensitive data in the log output.
