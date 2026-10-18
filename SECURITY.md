# Security Policy

## Supported versions

| Version line | Security support |
| ------------ | ---------------- |
| `1.x`        | Supported        |
| `0.x`        | Unsupported      |

Security fixes target the latest supported `1.x` release. No stable security-fix
commitment exists for `0.x`.

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

Guard inputs require repository-relative paths without traversal or symlinks.
Invalid paths are input errors, not probabilistic fail-open decisions.
Jev validates every path before reading evaluation content or sending a request.
Missing proposed files remain supported. The default build makes no Jev HTTP requests.

Git diff guard modes inspect path metadata only.
They enforce containment and protected paths without applying content-read exclusions.

## Support boundaries

Supported v1 distributions are the npm wrapper on macOS ARM64/x64 and Linux ARM64/x64.
Native Windows is unsupported in v1; use the documented
[WSL installation path](https://github.com/vixygrey/truenorth-mcp/wiki/Install-and-connect#windows).
The wrapper requires Node.js 18 or newer, but TrueNorth-MCP does not provide security
support for end-of-life Node.js releases. Source builds require Rust 1.88 or newer. The
server supports MCP over stdio; it does not certify any vendor-specific MCP client.

Use GitHub Discussions for questions. Use the issue forms for reproducible non-security
defects. Use the private advisory channel above for security reports only.
