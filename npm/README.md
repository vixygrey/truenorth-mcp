# TrueNorth-MCP

TrueNorth-MCP is an active MCP runtime for spec-driven engineering discipline. This
package is the npm launcher for the native `truenorth-mcp` server binary.

## Bootstrap a project

From an empty project directory, run:

```bash
npx -y truenorth-mcp init --profile generic
```

The wrapper selects the matching native package, forwards the bundled skills to the runtime,
and creates a language-agnostic `.agent/`, `specs/`, and `skills/` workspace. It does not
create a language manifest, source tree, CI workflow, or verify command. `init` refuses to
overwrite an existing TrueNorth workspace.

## Connect an MCP client

Register the wrapper as a stdio MCP server:

```json
{
  "mcpServers": {
    "truenorth": {
      "command": "truenorth-mcp"
    }
  }
}
```

The server reads and writes the governed project's `.agent/` workspace. Bootstrap before
connecting, then configure the project's own `TRUENORTH_VERIFY_CMD` in the MCP client
environment before using lifecycle, task, and quality-gate tools.

## Documentation and support

- [Install and connect guide](https://github.com/vixygrey/truenorth-mcp/wiki/Install-and-connect)
- [Compatibility policy](https://github.com/vixygrey/truenorth-mcp/wiki/Compatibility)
- [GitHub Discussions](https://github.com/vixygrey/truenorth-mcp/discussions)
- [Security policy](https://github.com/vixygrey/truenorth-mcp/security/policy)

For source builds and contribution guidance, see the
[repository README](https://github.com/vixygrey/truenorth-mcp).
