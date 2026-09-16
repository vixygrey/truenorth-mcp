---
name: smoke-test
description: "Post-deploy health check against a live URL. Validates the HTTP status, the response content, and the critical endpoints. Run it standalone or as the final step of the deploy skill."
---

# Smoke Test

> **HARD GATE**: Do NOT run smoke-test against a URL that is not deployed yet. Always run `deploy` first, then smoke-test.
>
> **HARD GATE**: a failed smoke test means the deployment is broken. Do NOT mark a deploy successful until every smoke check passes.

Validate that a deployed application is healthy by running HTTP checks against a
live URL. Each check asserts the HTTP status, an optional body signal (a regex),
and an optional response-time threshold.

## Configuration

The smoke checks live in `smoke-checks.yaml` at the project root:

```yaml
base_url: "https://example.com"
checks:
  - name: "Homepage"
    path: "/"
    expected_status: 200
    content_signal: "welcome|ok"
    max_response_time_ms: 3000
```

| Field                  | Required | Default | Description                            |
| ---------------------- | -------- | ------- | -------------------------------------- |
| `name`                 | Yes      | none    | A human-readable check name            |
| `path`                 | Yes      | `/`     | The URL path relative to `base_url`    |
| `method`               | No       | `GET`   | The HTTP method                        |
| `expected_status`      | No       | `200`   | The expected HTTP status code          |
| `content_signal`       | No       | none    | A regex or string in the response body |
| `max_response_time_ms` | No       | none    | Fail when slower than the threshold    |

## Process

### 1. Load the checks

Read the checks file, or a single ad-hoc URL from the environment. When neither a
checks file nor a URL is present, report the error and stop.

### 2. Run each check

Perform the HTTP request per check. Record pass or fail per assertion, and print a
summary.

### 3. Assert the results

- An HTTP status mismatch is a FAIL.
- A missing `content_signal` when one is configured is a FAIL.
- A response time over `max_response_time_ms` is a FAIL.
- A non-zero exit means the deployment is not healthy.

### 4. Generate the report

Capture the summary as evidence. Persist it for release-branch.

## Verify arc

Part of VERIFY: `verify-work`, then `validate-contracts`, then `smoke-test`, then
`run-evals`, then `audit-code`.

## Verify

Run the smoke checks against the deployed URL through the `truenorth_verify_gate`
tool. A pass returns exit 0.
