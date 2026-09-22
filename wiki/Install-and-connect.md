# Install and connect

This page installs the runtime and connects an MCP client to it over stdio.

## Install the wrapper

Install the `truenorth-mcp` npm package. The wrapper resolves the platform binary and runs
it. There is no business logic in the wrapper.

The wrapper ships per-platform binary packages under `optionalDependencies`, so the
package manager fetches only the binary for your platform (darwin-arm64, darwin-x64,
linux-arm64, or linux-x64).

## Verify a native release download

Each GitHub Release includes platform-native archives and a `SHA256SUMS` file. Download both
files, then verify the archive before extracting it:

```bash
shasum -a 256 -c SHA256SUMS
tar -xzf truenorth-mcp-v<version>-<platform>.tar.gz
```

The release also links every npm package version. After installing from npm, verify registry
signatures and provenance attestations with `npm audit signatures`.

## Connect an MCP client

Point your MCP client at the wrapper as a stdio server. The client runs the wrapper
command, and the two speak MCP over stdin and stdout. A typical client config names the
command and its arguments.

```json
{
  "mcpServers": {
    "truenorth": {
      "command": "npx",
      "args": ["truenorth-mcp"]
    }
  }
}
```

## Confirm the handshake

On `initialize`, the server reports its identity. A correct handshake returns the server
name `truenorth-mcp`, the version, the protocol version, and the `resources` and `tools`
capabilities.

To confirm by hand, send one `initialize` request over stdio:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"0"}}}' | npx truenorth-mcp
```

The result names `truenorth-mcp` and lists the two capabilities.

## Diagnose setup

Run these commands from the shell before adding the server to an MCP client:

```bash
truenorth-mcp --version
truenorth-mcp --check-config
```

The configuration check returns a JSON report with the repository root, workspace layout,
enabled features, verify-gate readiness, package version, and platform. It does not start
the MCP server or run the verify command. The report does not include configured command
values or other secret environment values.

## The repository root

The runtime governs one repository. It resolves the repository root by looking for a
directory that contains `.agent/`, `specs/`, and `skills/`. Run the client from the
repository root, or set the `TRUENORTH_ROOT` environment variable to the repository root.

A run from a subdirectory without `TRUENORTH_ROOT` fails to resolve the root. See
[Troubleshooting](Troubleshooting) for the exact error and the fix.

## Configure the verify gate

`truenorth_verify_gate` accepts only a lifecycle `phase` and runs a project command that the
MCP client operator configures. It passes only when the server observes exit code `0`.

Set the configuration in the MCP client environment, then restart the client after changing
it:

```json
{
  "mcpServers": {
    "truenorth": {
      "command": "npx",
      "args": ["truenorth-mcp"],
      "env": {
        "TRUENORTH_ROOT": "/absolute/path/to/repo",
        "TRUENORTH_VERIFY_CMD": "cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test",
        "TRUENORTH_GATE_ALLOWLIST": "cargo"
      }
    }
  }
}
```

`TRUENORTH_ROOT` is optional when the client starts in the repository root.
`TRUENORTH_VERIFY_CMD` is required and must name the project's verify or test command.
The command's first executable is automatically allowlisted.
`TRUENORTH_GATE_ALLOWLIST` is an optional comma-separated extension to that allowlist.
It checks only the first command token and is not a security boundary for shell fragments
such as `&&`, pipes, or redirects. The command is trusted operator configuration and never
comes from an MCP tool argument.

Call the tool with only the phase:

```json
{
  "phase": "review"
}
```

The successful result includes `"mode": "execute"` as output. `mode` and `test_evidence`
are not valid input fields.

## Confirm the layout contract

At startup the runtime validates the `.agent/` layout contract. A complete contract logs a
debug line that the layout validated. An incomplete contract logs a warning, and the
runtime keeps serving on the last valid state. See [The .agent workspace](The-agent-workspace)
for the required entries.

## Scaffold a new project

For a new project, call the `truenorth_scaffold_project` tool from your client. It seeds
the `.agent/` tree, the root workflow docs, the git hooks, and the `.github/` templates for
a methodology profile. It is non-destructive: it skips a path that already exists and
reports the skip. See [Methodology profiles](Methodology-profiles) for the profile choice.
