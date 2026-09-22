# TrueNorth-MCP

TrueNorth-MCP is an active MCP runtime for spec-driven engineering discipline. This
package is the npm launcher for the native `truenorth-mcp` server binary.

## Install

```bash
npm install truenorth-mcp
```

The wrapper selects the matching native package automatically. It supports macOS
ARM64 and x64 plus Linux ARM64 and x64. Windows is not supported. Node.js 18 or
newer is required.

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

The server reads and writes the governed project's `.agent/` workspace. Start by
calling `truenorth_scaffold_project` from your MCP client, then use the lifecycle,
task, and quality-gate tools for project work.

## Documentation and support

- [Install and connect guide](https://github.com/vixygrey/truenorth-mcp/wiki/Install-and-connect)
- [Compatibility policy](https://github.com/vixygrey/truenorth-mcp/wiki/Compatibility)
- [GitHub Discussions](https://github.com/vixygrey/truenorth-mcp/discussions)
- [Security policy](https://github.com/vixygrey/truenorth-mcp/security/policy)

For source builds and contribution guidance, see the
[repository README](https://github.com/vixygrey/truenorth-mcp).
