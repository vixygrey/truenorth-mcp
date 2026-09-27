# Install and connect

This page installs the runtime and connects an MCP client to it over stdio.

## Install the wrapper

Install the `truenorth-mcp` npm package. The wrapper resolves the platform binary and runs
it. There is no business logic in the wrapper.

The wrapper ships per-platform binary packages under `optionalDependencies`, so the
package manager fetches only the binary for your platform (darwin-arm64, darwin-x64,
linux-arm64, or linux-x64).

From an empty project directory, bootstrap the language-agnostic TrueNorth workspace:

```bash
npx -y truenorth-mcp init --profile generic
```

The default installation contains the always-selected `core` set. Inspect the catalog and
select optional sets independently of the methodology profile:

```bash
npx -y truenorth-mcp skills list
npx -y truenorth-mcp init --profile generic --skill-set integrations --skill-set visual
```

`integrations` requires configured external MCP or issue tracker services. `visual` may
require Node.js, Python, Chrome, or Puppeteer. `maintainer` is for TrueNorth repository
maintenance and requires the versioned wiki sources.

`init` creates `.agent/`, `specs/`, and `skills/` from the selected catalog. It refuses to
overwrite an existing TrueNorth workspace. It does not create a language manifest, source
tree, CI workflow, or verify command.

## Windows

Native Windows is unsupported in v1. Use WSL 2 so the npm wrapper and native Linux
binary run in the same environment.

1. Install WSL 2 from an elevated PowerShell terminal with `wsl --install`.
2. Open the Linux distribution and install Node.js 18 or newer. Use a Node.js release
   that still receives upstream security fixes for production.
3. Clone or open the governed project inside WSL and run `npx -y truenorth-mcp init`
   there.
4. Run the MCP client inside WSL, or configure it to invoke the wrapper through WSL
   with a Linux `TRUENORTH_ROOT` path.

The wrapper does not publish or attempt to resolve a native Windows binary.

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

### One mutating server per worktree

The first TrueNorth server for a worktree owns its writer lease. Other servers can read
existing resources, but their mutating tools return `writer_lease_conflict`. Close the
original server to transfer mutation rights, or give each parallel agent a separate Git
worktree and set `TRUENORTH_ROOT` to that worktree.

The lease path is `.agent/runtime/writer.lock`. Do not delete it to force ownership. The
operating-system lock, not the file's presence, controls ownership.

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
npx -y truenorth-mcp --version
npx -y truenorth-mcp --check-config
```

The configuration check returns a JSON report with the repository root, workspace layout,
enabled features, verify-gate readiness, allowed gate environment names, package version,
and platform. It does not start the MCP server or run the verify command. The report does
not include configured command or environment values.

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
      "args": ["-y", "truenorth-mcp"],
      "env": {
        "TRUENORTH_ROOT": "/absolute/path/to/repo",
        "TRUENORTH_VERIFY_CMD": "<your-project-verify-command>",
        "TRUENORTH_GATE_ENV_ALLOWLIST": "JAVA_HOME,CARGO_HOME"
      }
    }
  }
}
```

`TRUENORTH_ROOT` is optional when the client starts in the repository root.
`TRUENORTH_VERIFY_CMD` is required and must name the project's verify or test command.
The command's first executable is automatically allowlisted.
`TRUENORTH_GATE_ALLOWLIST` is an optional comma-separated extension to the command
allowlist. It checks only the first command token and is not a security boundary for shell
fragments such as `&&`, pipes, or redirects. The command is trusted operator configuration
and never comes from a verify-tool argument.

The bounded executor clears the inherited environment, then restores `PATH`, `HOME`,
`TMPDIR`, `TMP`, and `TEMP` when present. Add other safe names with the optional
comma-separated `TRUENORTH_GATE_ENV_ALLOWLIST`. Credential-like names, SSH agent variables,
and `TRUENORTH_*` names are rejected. Run `--check-config` after changing the list, then
restart the client. The diagnostic report shows names only.

This is not an operating-system sandbox. The command retains normal filesystem and network
access. Environment values are not included in diagnostics, and exact inherited values are
redacted from returned stderr.

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

## Scaffold an existing project

For an existing governed repository, call the `truenorth_scaffold_project` tool from your
client. It seeds missing `.agent/` files, root workflow docs, git hooks, and `.github/`
templates for a methodology profile without overwriting existing paths. For a fresh project,
use `npx -y truenorth-mcp init [--profile <name>]` before connecting an MCP client.
