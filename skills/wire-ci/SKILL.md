---
name: wire-ci
description: 'CI pipeline setup with forge-neutral guidance and local validation. Detects the forge from the git remote, generates a workflow for a supported forge, and skips honestly for the rest. The CI counterpart of wire-observability.'
---

# Wire CI

> **HARD GATE** (supported forges only): Do NOT ship a project without CI. Run this skill before the first merge to main.
>
> On a forge with no template, this skill is not a gate. It reports the forge, explains what it cannot do, and stops. A gate that cannot run must not claim it did. See the unsupported-forges section.
>
> **HARD GATE**: CI that is untestable locally breaks every cycle. Always validate the workflow after generating it, and dry-run it before pushing.

Generate, validate, and test a CI workflow. Detect the forge and the project type,
apply a stack-appropriate template, and verify locally before anything reaches CI.

## Forge resolution

Resolve the forge, first match wins: an explicit forge setting, then `forge:` in
`specs/forge.yaml`, then the `origin` remote URL, else `unknown`.

GitHub has templates under `.github/workflows/`. GitLab, Bitbucket, Codeberg, and
Gitea are detected but unsupported. For an unsupported forge, write nothing and
report the options.

## Template source

Keep the templates in the repository, so there is no network dependency on a third
party. Lay them out as `<root>/<forge>/test-build-release-<stack>.yml`.

## What this sets up

1. A test-build-release workflow: lint, then test, then build, then release, in one
   `needs:` chain.
2. Validation: YAML syntax, the workflow permissions, the required secrets, and
   common pitfalls.
3. A local dry run before the push.
4. The failure-pattern documentation below.

A deploy workflow is not bundled. It is platform-specific. See
[REFERENCE.md](REFERENCE.md) for a worked example.

## Process

### 1. Detect the forge and the stack

Read the project root to determine the stack.

| Manifest                       | Stack  | Template                        |
| ------------------------------ | ------ | ------------------------------- |
| `Cargo.toml`                   | Rust   | `test-build-release-rust.yml`   |
| `package.json`                 | Node   | `test-build-release-node.yml`   |
| `pyproject.toml` or `setup.py` | Python | `test-build-release-python.yml` |
| `go.mod`                       | Go     | `test-build-release-go.yml`     |

With no recognized manifest, stop and report the manifests you looked for. Do not
guess.

### 2. Apply the template

Do NOT rename the workflow `name:` field. The deploy workflow listens for the name
"Test Build Release". After copying, edit the placeholders: the language versions,
the app type, the site URL.

### 3. Unsupported forges

Write nothing. Report the options: provide a template for the forge, pin
`forge: github` in `specs/forge.yaml` when the remote is misdetected, or write the
CI config by hand.

### 4. Validate the workflow

Confirm the YAML syntax, the permissions, and the required secrets. Run the
validation through the `truenorth_verify_gate` tool so a clean result is an exit-0
verdict.

### 5. Dry-run the workflow

Run the workflow locally before the push. A local runner in a container is the most
accurate pre-push validation. A remote workflow run does not execute locally.

### 6. Common CI failure patterns

| Failure                         | Cause                                           | Fix                                              |
| ------------------------------- | ----------------------------------------------- | ------------------------------------------------ |
| `npm publish` fails             | The npm token is not set as a repo secret       | Add the token to the repo secrets                |
| A release job cannot write      | Missing `permissions: contents: write`          | Add it to the release job                        |
| `cargo publish` auth fails      | The cargo registry token is not set             | Add the token to the env or the cargo config     |
| `go vet` fails                  | A Go version mismatch                           | Use `go-version-file: go.mod`                    |
| `cargo clippy` errors           | New nightly lints                               | Pin the toolchain, then `cargo clippy --fix`     |
| The local runner is not found   | Docker is not running, or the runner is missing | Install the runner, confirm Docker is up         |
| A stale hardcoded Node version  | An `.nvmrc` exists but the workflow hardcodes   | Use `node-version-file: .nvmrc`                  |
| Deploy never runs               | The workflow was renamed                        | Keep `name: Test Build Release`                  |
| The release rebuilds the binary | The artifact was not downloaded                 | The release job must download the build artifact |

## Verify

Run the workflow validation through the `truenorth_verify_gate` tool. A pass
returns exit 0.
