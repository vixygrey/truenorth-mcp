# Security Policy

## Supported versions

| Version line | Security support |
| ------------ | ---------------- |
| `2.x`        | Supported        |
| `1.x`        | Unsupported      |
| `0.x`        | Unsupported      |

Security fixes target the latest supported `2.x` release. No stable security-fix
commitment exists for older release lines.

## Report a vulnerability privately

Do not open a public issue for a suspected vulnerability. Submit a private GitHub Security
Advisory instead:

<https://github.com/vixygrey/truenorth-mcp/security/advisories/new>

Include the affected version or commit, impact, reproduction steps, and a minimal proof of
concept when safe to share. Do not include access tokens, credentials, customer data,
private `.agent/` workspace contents, or other secrets.

The maintainer will acknowledge a private report within seven calendar days. After triage,
the maintainer will coordinate remediation and disclosure with the reporter. Resolved
advisories name the reporter only with their consent.

## Security controls

TrueNorth-MCP uses these controls:

- GitHub Dependabot alerts and weekly dependency and GitHub Actions updates.
- GitHub CodeQL default setup.
- A weekly OSV-Scanner dependency scan that uploads SARIF results to GitHub Code Scanning.
- The independent `guard-git` Git credential scanner checks added content in pre-commit
  and CI; it does not provide the retired path-metadata guard policy.
- npm OIDC trusted publishing, provenance, staging, and maintainer 2FA approval.

These controls reduce risk. They do not guarantee that every vulnerability is detected or
that every supported configuration is secure.

Merge, release, and emergency-bypass authority are defined in [GOVERNANCE.md](GOVERNANCE.md).

## Filesystem boundaries

Resource and skill reads reject symlinked files and parent directories beneath the repository root.
The repository root itself may be a symlink. Reads exclude private paths and `.agent/telemetry/`.
Protected operations use directory descriptors, so replacing a path with a symlink cannot redirect an open.

Cockpit recovery validates every journal entry and target revision before changing any migration target.
Only `state`, `release-plan`, and `bugs` files under `.agent/tasks/`, with `.yml` or `.json` extensions, are permitted.
Recovery acquires the worktree writer lease and rejects symlinked journals, targets, and parent directories.

## Support boundaries

Supported distributions are the npm wrapper on macOS ARM64/x64 and Linux ARM64/x64.
Native Windows is unsupported; use the documented
[WSL installation path](https://github.com/vixygrey/truenorth-mcp/wiki/Install-and-connect#windows).
The wrapper requires Node.js 18 or newer, but TrueNorth-MCP does not provide security
support for end-of-life Node.js releases. Source builds require Rust 1.88 or newer. The
server supports MCP over stdio; it does not certify any vendor-specific MCP client.

Use GitHub Discussions for questions. Use the issue forms for reproducible non-security
defects. Use the private advisory channel above for security reports only.
