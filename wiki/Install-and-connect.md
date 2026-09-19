# Install and connect

This page installs the runtime and connects an MCP client to it over stdio.

## Install the wrapper

Install the `truenorth-mcp` npm package. The wrapper resolves the platform binary and runs
it. There is no business logic in the wrapper.

The wrapper ships per-platform binary packages under `optionalDependencies`, so the
package manager fetches only the binary for your platform (darwin-arm64, darwin-x64,
linux-arm64, or linux-x64).

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

## The repository root

The runtime governs one repository. It resolves the repository root by looking for a
directory that contains `.agent/`, `specs/`, and `skills/`. Run the client from the
repository root, or set the `TRUENORTH_ROOT` environment variable to the repository root.

A run from a subdirectory without `TRUENORTH_ROOT` fails to resolve the root. See
[Troubleshooting](Troubleshooting) for the exact error and the fix.

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
