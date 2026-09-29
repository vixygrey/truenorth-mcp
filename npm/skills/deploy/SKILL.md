---
name: deploy
description: "Build, verify the artifact, deploy, wait, then smoke the deployment. Platform-agnostic (MCP or CLI), with a configurable timeout, retry with exponential backoff, and an integrated health check. The deploy half of CI/CD: run it after the build to push to production."
kind: prose
name_exception: Single-verb operational skill.
---

# Deploy

> **HARD GATE** — Do not deploy without running tests first. Run `test` or your CI suite before this skill.
>
> **HARD GATE** — Use this skill from a CI/CD pipeline or post-merge on `main`/`master`. Never deploy from a feature branch.
>
> **HARD GATE** — The deploy skill orchestrates deployment; the `smoke-test` skill validates post-deploy health. Chain them: `deploy → smoke-test`.
>
> **HARD GATE** — Explicit approval before production mutation: prompt the user with the resolved target platform, destination URL/host, and artifact path. Never trigger a production deployment without human confirmation.
>
> **HARD GATE** — Never pass raw authorization tokens as command-line arguments where they can be observed in process lists (`ps`). Redact credentials and auth headers from all deployment logs.
> Orchestrate a full build-to-deployment pipeline: build the artifact, verify it exists and is non-empty, invoke a platform deploy tool (MCP or CLI), poll until the deploy completes or times out, then run a baseline smoke test against the live URL.

## Pipeline Stages

```
build → verify artifact → deploy → wait/retry → smoke
```

| Stage  | Description                                                         | Failure mode                        |
| ------ | ------------------------------------------------------------------- | ----------------------------------- |
| Build  | Execute the project's build command                                 | Non-zero exit: report build error   |
| Verify | Check artifact exists and is non-empty                              | Missing/empty: report artifact path |
| Deploy | Invoke platform deploy tool (MCP, Vercel CLI, rsync, etc.)          | Non-zero exit: report deploy error  |
| Wait   | Poll deploy status every 30s up to `DEPLOY_TIMEOUT` (default 5 min) | Timeout: report exceeded            |
| Smoke  | `curl -sSf $DEPLOY_URL` as baseline health check                    | Non-200: report failure             |

## Process

### 1. Detect build command

Read project manifest files in order to determine the build command:

| Manifest                      | Build command                                                       |
| ----------------------------- | ------------------------------------------------------------------- |
| `package.json`                | `npm run build` (or `scripts.build` value)                          |
| `Cargo.toml`                  | `cargo build --release`                                             |
| `pyproject.toml` / `setup.py` | Depends on build backend (`poetry build`, `pip install -e .`, etc.) |
| `Makefile`                    | `make build` or first target named `build`                          |
| The project agent guide       | Look for `build:` in the project commands section                   |

If no manifest is found, prompt the user with: "No detected build command. Pass `--build 'npm run build'` or specify the command."

### 2. Build the artifact

```bash
npm run build
```

Or the detected command from step 1. If the build fails, exit non-zero and report the build output.

### 3. Verify the artifact

```bash
ARTIFACT_DIR="${ARTIFACT_DIR:-dist}"
if [ ! -d "$ARTIFACT_DIR" ] || [ -z "$(ls -A "$ARTIFACT_DIR" 2>/dev/null)" ]; then
  echo "FAIL: build artifact not found at $ARTIFACT_DIR"
  exit 1
fi
```

Configurable via `$ARTIFACT_DIR` environment variable (default: `dist/`).

### 4. Deploy to platform

Deploy target selection requires explicit configuration via `DEPLOY_TARGET`. Targets are not selected implicitly from ambient session variables.

| Target (`DEPLOY_TARGET`) | Required configuration                | Execution pattern                                                                  |
| ------------------------ | ------------------------------------- | ---------------------------------------------------------------------------------- |
| `vercel`                 | `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID`  | Vercel CLI reads config or env directly without CLI token flags                    |
| `netlify`                | `NETLIFY_SITE_ID`                     | Netlify CLI reads config or env directly without CLI auth flags                    |
| `mcp`                    | Configured platform MCP server        | Invoke platform deploy tool via MCP                                                |
| `rsync`                  | `DEPLOY_SSH_HOST`, `DEPLOY_SSH_PATH`  | `rsync -avz "$ARTIFACT_DIR/" "$DEPLOY_SSH_USER@$DEPLOY_SSH_HOST:$DEPLOY_SSH_PATH"` |
| `custom`                 | `DEPLOY_COMMAND` (reviewed with user) | Run pre-approved, non-interactive deploy script                                    |

If `DEPLOY_TARGET` is unset or unrecognized:

```bash
echo "Error: DEPLOY_TARGET must be explicitly set to one of: vercel, netlify, mcp, rsync, custom." >&2
exit 1
```

Do not run arbitrary, unreviewed command strings by default. Redact any tokens or secret keys from standard output and standard error before recording deployment receipts.

### 5. Wait and poll status

After invoking the deploy command, poll for completion:

See [REFERENCE.md](REFERENCE.md)

Use exponential backoff for retries on transient failures:

See [REFERENCE.md](REFERENCE.md)

### 6. Baseline smoke test

See [REFERENCE.md](REFERENCE.md)

For a comprehensive health check, chain to the `smoke-test` skill against the
deployed URL.

### 7. Three-independent-facts verification

Before you declare a deploy successful, verify three independent facts: the build
artifact, the platform accept, and the live or registry reachability. See
[REFERENCE.md](REFERENCE.md#three-independent-facts).

## Verify

→ verify: `command -v curl >/dev/null 2>&1 && test -f skills/smoke-test/SKILL.md`
